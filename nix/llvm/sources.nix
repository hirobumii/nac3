{fetchurl}: rec {
  version = "17.0.6";
  cmake = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/cmake-${version}.src.tar.xz";
    hash = "sha256-gH8GnFTcIMtHshwfasr92cZJ864BVgkEDWGCyrARQPQ=";
  };
  llvm = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/llvm-${version}.src.tar.xz";
    hash = "sha256-tjgWfaE5EmyhGRe2iAIHzG6PnRy7GkjYfQF/aX73gYg=";
  };
  clang = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/clang-${version}.src.tar.xz";
    hash = "sha256-p49minJq4dPZpxeZltl7ErkPt2q5RCpDEQuXL/etkCk=";
  };
  compiler-rt = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/compiler-rt-${version}.src.tar.xz";
    hash = "sha256-EbjQnc+SoPkcXILe+1rZ/0rPXPBzqAwxcgS6qSLRNrQ=";
  };
}
