use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{RequestedFormat, RequestedFormatType, Resolution};
use nokhwa::{native_api_backend, query, Camera};

const MAX_FRAME_SIZE: (u32, u32) = (1280, 720);

pub fn create_camera() -> Camera {
    let backend = native_api_backend().expect("Could not get backend");
    let devices = query(backend).expect("Could not query backend");
    let device = devices.first().expect("No devices found");

    let format =
        RequestedFormat::new::<RgbAFormat>(RequestedFormatType::HighestResolution(Resolution {
            width_x: MAX_FRAME_SIZE.0,
            height_y: MAX_FRAME_SIZE.1,
        }));

    Camera::new(device.index().to_owned(), format).expect("Could not create camera")
}
