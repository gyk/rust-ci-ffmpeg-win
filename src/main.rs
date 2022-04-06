use std::path::PathBuf;

use anyhow::Result;
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::Flags;
use ffmpeg::util::frame::video::Video;
use image::ImageBuffer;

fn main() -> Result<()> {
    ffmpeg::init()?;

    let args: Vec<String> = std::env::args().collect();
    let in_path = args[1].parse::<PathBuf>()?;
    let out_dir = &args[2];

    let mut input_ctx = ffmpeg::format::input(&in_path)?;

    for datum in input_ctx.metadata().iter() {
        println!("\t{}: {}", datum.0, datum.1);
    }

    println!("Duration: {}ms", input_ctx.duration() / 1000);
    println!("#Streams: {}", input_ctx.nb_streams());
    println!("#Chapters: {}", input_ctx.nb_chapters());

    for s in input_ctx.streams() {
        println!("Stream #{}", s.index());
        println!(
            "\tt = {}, fps = {}, rate = {}",
            s.time_base(),
            s.avg_frame_rate(),
            s.rate()
        );
        println!(
            "\tframe rate = {}",
            unsafe { *s.parameters().as_ptr() }.bit_rate
        );

        for datum in &s.metadata() {
            println!("\t\t{}: {}", datum.0, datum.1);
        }
    }

    let v_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;
    let v_index = v_stream.index();

    let decoder_ctx = ffmpeg::codec::Context::from_parameters(v_stream.parameters())?;
    let mut decoder = decoder_ctx.decoder().video()?;

    let width = decoder.width();
    let height = decoder.height();
    let out_width = width / 2;
    let out_height = height / 2;

    let mut scaler_ctx = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        width,
        height,
        Pixel::RGBA,
        out_width,
        out_height,
        Flags::BILINEAR,
    )?;

    for packet in input_ctx.packets().filter_map(|(stream, packet)| {
        if stream.index() == v_index {
            Some(packet)
        } else {
            None
        }
    }) {
        decoder.send_packet(&packet)?;

        let mut v_frame = Video::empty();
        if decoder.receive_frame(&mut v_frame).is_ok() {
            let mut rgb_frame = Video::empty();
            scaler_ctx.run(&v_frame, &mut rgb_frame)?;

            use image::Rgba;
            let im: ImageBuffer<Rgba<u8>, Vec<u8>> =
                ImageBuffer::from_vec(out_width, out_height, rgb_frame.data(0).to_vec())
                    .ok_or_else(|| anyhow::Error::msg("Cannot create ImageBuffer"))?;
            let out_path = format!("{}/output.jpg", out_dir);
            im.save(&out_path)?;
            return Ok(());
        }
    }

    Ok(())
}
