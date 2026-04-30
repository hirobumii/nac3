{fetchurl}: rec {
  version = "19.1.1";
  cmake = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/cmake-${version}.src.tar.xz";
    hash = "sha256-kqAW7P5GrXwY22QloBjCxu4Sa50OVRPW+tmJ/uZkj/o=";
  };
  llvm = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/llvm-${version}.src.tar.xz";
    hash = "sha256-FafHf5w5RE2d1nVrdbmnASncvR40BySm5Fs7SI9VvEs=";
  };
  clang = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/clang-${version}.src.tar.xz";
    hash = "sha256-c4gczwZcNcpndSwtS23QFXFAMw7vMY+4DxpiaBFFz3w=";
  };
  compiler-rt = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/compiler-rt-${version}.src.tar.xz";
    hash = "sha256-tj3G1iEHUusetC0oVJNGuzVDlRv70BQVLuxkj1261Kc=";
  };
}
