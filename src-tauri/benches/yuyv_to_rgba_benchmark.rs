use criterion::{black_box, criterion_group, criterion_main, Criterion};

use tauri_wgpu::camera::yuyv_to_rgba;

fn yuyv_to_rgba_benchmark(c: &mut Criterion) {
    // HD resolution: 1280x720
    const WIDTH: usize = 1280;
    const HEIGHT: usize = 720;

    // YUYV format: 2 bytes per pixel (Y U Y V pattern covers 2 pixels in 4 bytes)
    let yuyv_buffer_size = WIDTH * HEIGHT * 2;
    let yuyv_buffer = vec![128u8; yuyv_buffer_size];

    c.bench_function("yuyv_to_rgba HD (1280x720)", |b| {
        b.iter(|| yuyv_to_rgba(black_box(&yuyv_buffer), black_box(WIDTH), black_box(HEIGHT)))
    });
}

criterion_group!(benches, yuyv_to_rgba_benchmark);
criterion_main!(benches);
