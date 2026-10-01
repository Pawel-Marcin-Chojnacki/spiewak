fn main() {
    let root = "native/chromaprint/src";
    println!("cargo:rerun-if-changed=native/chromaprint");
    let out = std::env::var("OUT_DIR").expect("Cargo output directory");
    std::fs::write(format!("{out}/config.h"), "#define HAVE_ROUND 1\n#define HAVE_LRINTF 1\n#define USE_KISSFFT 1\n#define USE_INTERNAL_AVRESAMPLE 1\n").expect("write Chromaprint configuration");
    cc::Build::new()
        .include(root)
        .include(&out)
        .include(format!("{root}/3rdparty/kissfft"))
        .define("HAVE_CONFIG_H", None)
        .define("_USE_MATH_DEFINES", None)
        .files([
            format!("{root}/avresample/resample2.c"),
            format!("{root}/3rdparty/kissfft/kiss_fft.c"),
            format!("{root}/3rdparty/kissfft/kiss_fftr.c"),
        ])
        .warnings(false)
        .compile("chromaprint_c");
    let files = [
        "audio_processor",
        "chroma",
        "chroma_resampler",
        "chroma_filter",
        "spectrum",
        "fft",
        "fingerprinter",
        "image_builder",
        "simhash",
        "silence_remover",
        "fingerprint_calculator",
        "fingerprint_compressor",
        "fingerprint_decompressor",
        "fingerprinter_configuration",
        "fingerprint_matcher",
        "utils/base64",
        "fft_lib_kissfft",
        "chromaprint",
    ];
    cc::Build::new()
        .cpp(true)
        .include(root)
        .include(&out)
        .include(format!("{root}/3rdparty/kissfft"))
        .define("HAVE_CONFIG_H", None)
        .define("CHROMAPRINT_NODLL", None)
        .define("_USE_MATH_DEFINES", None)
        .flag_if_supported("/std:c++14")
        .flag_if_supported("-std=c++14")
        .files(files.map(|f| format!("{root}/{f}.cpp")))
        .warnings(false)
        .compile("chromaprint");
}
