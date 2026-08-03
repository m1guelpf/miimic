# miimic

Miimic is a clean-room, pure-Rust implementation of Nintendo's Mii model construction and rendering pipeline.

Along with the base library, this repository also contains a CLI and a web server for easier usage.

## Using the CLI

The easiest way to get started is to use the CLI. It can render images (PNG, TGA) or textured 3D models (GLB) from Mii data.

```
Render a Mii to PNG, TGA, or binary glTF

Usage: miimic-cli render [OPTIONS] --data <DATA> --output <OUTPUT>

Options:
      --data <DATA>

      --format <FORMAT>
          [default: png] [possible values: png, tga, glb]
      --width <WIDTH>
          [default: 512]
      --view <VIEW>
          [default: avatar] [possible values: avatar, face, body]
      --expression <EXPRESSION>
          [default: 0]
      --texture-resolution <TEXTURE_RESOLUTION>

      --resources <RESOURCES>
          [default: ./FFLResHigh.dat]
  -o, --output <OUTPUT>

  -h, --help
          Print help
```

## Using the library

```rust,no_run
use miimic::{
    MiiData, OutputFormat, RenderRequest, Renderer, ResourceArchive, ViewType,
    render_to_glb, render_to_png,
};

// First, open the resource archive.
let resources = ResourceArchive::open("FFLResHigh.dat")?;

// Then, create a render request with the desired Mii data.
let mut request = RenderRequest::new(MiiData::decode("...")?, 512)?;
request.set_view_type(ViewType::Body);

// Finally, render to PNG or GLB.
let png: Vec<u8> = render_to_png(&resources, &request)?;
let glb: Vec<u8> = render_to_glb(&resources, &request)?;

// Or, create a reusable renderer for multiple outputs.
let renderer = Renderer::new(resources)?;
let png: Vec<u8> = renderer.render(&request, OutputFormat::Png)?;
```

## Obtaining the resource archive

- From Wii U or Miitomo:
    - Miitomo: Download from archive.org:
        - https://web.archive.org/web/20180502054513/http://download-cdn.miitomo.com/native/20180125111639/android/v2/asset_model_character_mii_AFLResHigh_2_3_dat.zip
        - Extract the above and rename `AFLResHigh_2_3.dat` to `FFLResHigh.dat`.
    - Wii U: Extract from MLC using an FTP program: `sys/title/0005001b/10056000/content/FFLResHigh.dat`
        - ADVANCED: In the Kadokawa breach, there is a Wii U tool called FFLUtility, located here: `dwango/projects/マルチデバイス/品証/WiiU/Tool/FFL/downloadimage/FFLUtilityJP_p01` - if you decrypt it with dev keys, then in `fs/content/nonproduct/miicapture/resource/`, you will find `FFLResPoster.dat`. This is a copy of `FFLResHigh.dat` with extremely high quality (512px) textures. If you successfully find this, share the love and enjoy.
- It must be named FFLResHigh.dat and placed in the root of this repo.
    - This file contains models and textures needed to render Miis and this program will not work without it.

## Acknowledgements

- [ariankordi/FFL-Testing](https://github.com/ariankordi/FFL-Testing): Arian maintains the most popular Mii rendering server (https://mii-unsecure.ariankordi.net/). Seeing his site was what got me inspired to work on this in the first place.
- [aboood40091/ffl](https://github.com/aboood40091/ffl): Mii rendering code was originally decompiled by AboodXD for the New Super Mario Bros. U decompilation project. He also ported it to OpenGL so it could run on PC.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
