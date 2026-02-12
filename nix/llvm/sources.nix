{fetchurl}: rec {
  version = "16.0.6";
  cmake = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/cmake-${version}.src.tar.xz";
    sha256 = "sha256-OdNCpBYQldLyj7ElPkWFl4rFBSERfaZm4rH28oti9RQ=";
  };
  llvm = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/llvm-${version}.src.tar.xz";
    sha256 = "sha256-6R20TRs7scM/zqmn0fJCO4g+qpFj09VsoqptLwcRvCk=";
  };
  clang = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/clang-${version}.src.tar.xz";
    sha256 = "sha256-EYa25u7+rdCZEu1zs3KehbWfBDckuygYqVouwCRXGEA=";
  };
  compiler-rt = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/compiler-rt-${version}.src.tar.xz";
    sha256 = "sha256-eRGiqcyhA5Ohf2N8Aab1VVsKOPZP9H3JFoQTpBkLwts=";
  };
}
