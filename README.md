# Tauri v2 + wgpu + nokhwa



https://github.com/user-attachments/assets/f47a2cb5-2b04-474a-b706-05744adae437



> **Warning**
> This project is currently only compatible with **macOS**. Windows and Linux support is not implemented.

A demonstration of efficient camera frame rendering using Tauri v2 combined with WebGPU (wgpu) and the nokhwa camera library. This project renders camera frames directly to native windows using GPU textures, avoiding the overhead of Tauri's IPC or WebSocket approaches.


## Rendering Modes

This fork adds an example of toggling between two camera display modes:

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

- macOS camera format reporting can be inconsistent
- Frame decoding is CPU-based, which may be a performance bottleneck
- Application architecture could be further refined

Pull requests addressing these issues are welcome.

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
- [Learn wgpu tutorial](https://sotrh.github.io/learn-wgpu/)
