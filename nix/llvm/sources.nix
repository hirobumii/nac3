{fetchurl}: rec {
  version = "18.1.8";
  cmake = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/cmake-${version}.src.tar.xz";
    hash = "sha256-Wbre9ZLdNIk80xnUKzI6qpkLRS0FxxgP8g8jqxtB6Dc=";
  };
  llvm = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/llvm-${version}.src.tar.xz";
    hash = "sha256-9oz5Dzabx9AVi6cNhgsMs028Fj1v8OvGz6XlFbmy4o0=";
  };
  clang = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/clang-${version}.src.tar.xz";
    hash = "sha256-VyT+ChMIfVV5EEzt0vizvBCiEvt5oPzayY9IgOGfRRk=";
  };
  compiler-rt = fetchurl {
    url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/compiler-rt-${version}.src.tar.xz";
    hash = "sha256-4FTpmpySQHIGFukny1I2OrvItPHvAoa609957I/fiS8=";
  };
}
