use barkfart_ffi::*;
use std::ffi::CString;

#[test]
fn exports_gif_png_frames_and_soundtrack_through_the_c_interface() {
    let e = bf_engine_new();
    if e.is_null() {
        eprintln!("skipping: no GPU");
        return;
    }
    let bark = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/ref/sound-demo.bark")).unwrap();
    assert_eq!(bf_project_load(e, bark.as_ptr(), bark.len()), 0);
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let run = |format: i32, name: &str| -> (std::path::PathBuf, i32) {
        let path = dir.join(name);
        let c = CString::new(path.to_str().unwrap()).unwrap();
        let job = bf_export_begin(e, format, c.as_ptr(), 160, 90, 0, 0, 1);
        assert!(!job.is_null());
        let count = bf_export_count(job);
        loop {
            let r = bf_export_step(e, job);
            assert_ne!(r, -2, "export failed");
            if r == -1 {
                break;
            }
        }
        bf_export_free(job);
        (path, count)
    };
    let (gif, frames) = run(0, "demo.gif");
    assert_eq!(frames, 60, "2 s at 30 fps");
    let g = std::fs::read(&gif).unwrap();
    assert_eq!(&g[..6], b"GIF89a");
    assert_eq!(*g.last().unwrap(), 0x3B);
    let dec_gif = barkfart_core::image::decode_gif(&g).expect("decoded gif");
    assert!(dec_gif.frames[0].rgba.chunks_exact(4).any(|p| p[3] == 0), "transparent GIF background");
    let (zip_path, _) = run(1, "demo.zip");
    let mut z = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
    assert_eq!(z.len(), 60);
    let mut f = z.by_name("frame_0001.png").unwrap();
    let mut b = Vec::new();
    std::io::Read::read_to_end(&mut f, &mut b).unwrap();
    let img = barkfart_core::image::decode(&b).unwrap();
    assert_eq!((img.w, img.h), (160, 90));
    assert!(img.px.chunks_exact(4).any(|p| p[3] == 0), "transparent background");
    assert!(img.px.chunks_exact(4).any(|p| p[3] == 255 && p[0] < 40), "the black figure");
    let wav = dir.join("demo.wav");
    let c = CString::new(wav.to_str().unwrap()).unwrap();
    assert_eq!(bf_export_soundtrack(e, c.as_ptr(), 0, 0), 0);
    let pcm = barkfart_core::audio::decode_wav(&std::fs::read(&wav).unwrap()).unwrap();
    assert!((pcm.seconds() - 2.0).abs() < 1e-3);
    assert!(pcm.data.iter().any(|s| s.abs() > 3000), "beeps");
    // a cancelled export leaves no file
    let gone = dir.join("cancel.gif");
    let c = CString::new(gone.to_str().unwrap()).unwrap();
    let job = bf_export_begin(e, 0, c.as_ptr(), 64, 36, 0, 0, 0);
    bf_export_step(e, job);
    bf_export_free(job);
    assert!(!gone.exists(), "cancelled export removed");
    bf_engine_free(e);
}
