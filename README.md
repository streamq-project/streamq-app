> [!IMPORTANT]
> This project is source-available, not open source. The code may not be used to provide a product that competes with StreamQ, including free or open-source substitutes.
> See [LICENSE.md](LICENSE.md) for the controlling terms.

# StreamQ App

StreamQ App is an Electron application that enhances the capabilities of the web application streamq.io.

## Prerequisites

Before you begin, ensure you have the following:

- Windows 10 or later, or Linux
- Node.js 21 or later
- pnpm 11+
- Rust, installed through [rustup](https://rustup.rs/)
- On Windows: follow the [node-gyp Windows setup instructions](https://github.com/nodejs/node-gyp#on-windows) to install the required build tools
- On Linux, when building for Linux: a C/C++ toolchain, `pkg-config`, and the development packages for GTK 3, GTK Layer Shell, WebKitGTK 4.1, and PulseAudio
- On Linux, when building for Windows x64: `clang` and the Windows MSVC Rust target:

  ```bash
  rustup target add x86_64-pc-windows-msvc
  ```

## Installation

Clone the repository and install the dependencies:
```bash
git clone https://github.com/streamq-project/streamq-app.git
cd streamq-app
pnpm i
```

## Usage

- `start`: Start the development server.
- `make:dev`: Create a development version.
- `make:prod`: Create a production version.
- `rebuild`: Rebuild the addons.
- `lint`: Check the code with ESLint.

The `package`, `make:dev`, and `make:prod` commands accept the Electron Forge `--platform` and `--arch` options when building for a different platform or architecture. For example: `--platform=win32 --arch=x64`.

For cross-platform Rust development, install the required Rust target and configure your editor to analyze that target. See the [rustup cross-compilation guide](https://rust-lang.github.io/rustup/cross-compilation.html) and the [`rust-analyzer.cargo.target` configuration](https://rust-analyzer.github.io/book/configuration#cargo.target).

## License

The source code of this client is available under the [**PolyForm Shield License 1.0.0**](LICENSE.md).

For API usage, please refer to the [StreamQ Terms of Service](https://streamq.io/terms).

Third-party software used by or referenced from this project remains under its own license terms; see [thirdparty/NOTICE.txt](thirdparty/NOTICE.md).
