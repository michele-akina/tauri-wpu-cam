# Tauri v2 + wgpu + nokhwa

<img width="1020" height="640" alt="Screenshot 2025-12-30 at 21 26 54" src="https://github.com/user-attachments/assets/b3b2d17b-bd1e-4070-970d-ae6498e61324" />


> **Warning**
> This project is currently only compatible with **macOS**. Windows and Linux support is not implemented.

A demonstration of efficient camera frame rendering using Tauri v2 combined with wgpu and nokwha. This project renders camera frames directly to native windows using GPU textures, avoiding the overhead of Tauri's IPC or WebSocket approaches. Ideal for applications that require processing of the camera frames in the Tauri backend before rendering. 


## Rendering Modes

This is a fork of [clearlysid/tauri-wgpu-cam](https://github.com/clearlysid/tauri-wgpu-cam) and adds an example of toggling between two camera display modes:

- **Thumbnail Mode**: Camera renders in a small overlay window positioned over the main window (default)
- **Background Mode**: Camera renders as the full background of the main window, with UI elements layered on top

## Development

### Prerequisites

1. Install [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
2. Install dependencies:
   ```bash
   yarn install
   ```

### Running

```bash
make dev
```

## Known Limitations

- macOS camera format reporting can be inconsistent with nokhwa (yuyv instead of requested rgba), and the decoding (yuyv_to_rgba) is currently CPU-based, which may be a performance bottleneck (see benchmark). If you do not need to do further processing on the frame, the smartest thing to do would be to add a compute shader for the conversion in the GPU command buffer, before the rendering. This would reduce CPU->GPU bandwidth (yuyv is smaller than rgba) and speed up the conversion significantly. The conversion is a textbook GPU task as it can be parallelized on each pixel
- We might lose the camera aspect ratio when resizing the window. Should be an easy fix
- Will probably not work on Windows and Linux. Most window operations are done with macOS-specific APIs

## Changes from Original

This fork includes the following updates:
- Updated dependencies, notably wgpu to v28 (includes breaking API changes)
- Added flume for async channels between camera and render loops
- Added tracing/tracing-subscriber for structured logging
- Implemented thumbnail/background mode toggle with surface switching
- Added window management utilities for overlay positioning and transparency

## Acknowledgments

This project is a fork of [clearlysid/tauri-wgpu-cam](https://github.com/clearlysid/tauri-wgpu-cam).

### Resources & Inspiration

- [FabianLars' Tauri + wgpu demo](https://github.com/FabianLars/tauri-v2-wgpu)
- [wgpu documentation](https://wgpu.rs/)
- [WebGPU Fundamentals](https://webgpufundamentals.org/)






