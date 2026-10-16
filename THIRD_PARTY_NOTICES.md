# Third-party notices

Kirk Companion code is MIT-licensed. Dependencies retain their open-source licenses; Cargo.lock identifies exact versions.

Main components: egui/eframe/wgpu, Tokio, reqwest/rustls, serde, keyring/platform credential stores, tray-icon, rfd, and image. Consult each component's license rather than treating this list as a substitute for its terms.

egui's embedded default fonts have separate notices, including Ubuntu Font Licence and SIL Open Font License. Packaging gathers dependency license files and embedded font notices into `THIRD-PARTY-LICENSES.txt`, installed beside the executable.

The primary UI face is Squares Bold, the exact font used by Kirk Client. Copyright (c) 2014 by Typetype; designer Ivan Gladkikh, technical designer Olexa Volochay. The unmodified file's embedded license identifies SIL OFL 1.1. Its notice and license are in `assets/fonts/` and are distributed with the app.

No assets from the user's inspiration images, proprietary mod runtime, or private account data are redistributed.
