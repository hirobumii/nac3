{
  lib,
  stdenv,
  wineWow64Packages,
  fetchurl,
  cmake,
  python3,
  ncurses,
  zlib,
  ninja,
  libxcrypt,
  msys2-env ? null,
  extraCmakeFlags ? [],
  enableProjects ? [],
  llvmTools ? ["llvm-config"],
  extraConfig ? "",
  runCommand,
  wrapCCWith,
}: let
  inherit (lib) optional optionals optionalString;
in rec {
  exe_suffix =
    if msys2-env == null
    then ""
    else ".exe";
  llvm = stdenv.mkDerivation rec {
    pname = "llvm-nac3";
    version = "22.1.8";
    src = fetchurl {
      url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/llvm-project-${version}.src.tar.xz";
      hash = "sha256-ki8YF6DfexSJJy0YE07gCHqLBogo+HrGO5hhsamWWIg=";
    };
    nativeBuildInputs =
      if msys2-env == null
      then [cmake python3 ninja]
      else [wineWow64Packages.stable];
    buildInputs =
      if msys2-env == null
      then [libxcrypt]
      else [];
    propagatedBuildInputs =
      if msys2-env == null
      then [ncurses zlib]
      else [];
    phases = ["unpackPhase" "patchPhase" "configurePhase" "buildPhase" "installPhase"];

    cmakeFlags =
      [
        "-DCMAKE_BUILD_TYPE=MinSizeRel"
        "-DLLVM_ENABLE_UNWIND_TABLES=OFF"
        "-DLLVM_ENABLE_THREADS=ON"
        "-DLLVM_TARGETS_TO_BUILD=X86\;ARM\;RISCV"
        "-DLLVM_LINK_LLVM_DYLIB=OFF"
        "-DLLVM_ENABLE_FFI=OFF"
        "-DFFI_INCLUDE_DIR=fck-cmake"
        "-DFFI_LIBRARY_DIR=fck-cmake"
        "-DLLVM_ENABLE_LIBXML2=OFF"
        "-DLLVM_INCLUDE_TESTS=OFF"
        "-DLLVM_INCLUDE_BENCHMARKS=OFF"
        "-DLLVM_BUILD_TOOLS=OFF"
        "-DCMAKE_INSTALL_PREFIX=${placeholder "out"}"
        ("-DLLVM_ENABLE_PROJECTS=" + (lib.strings.concatStringsSep "\;" enableProjects))
      ]
      ++ extraCmakeFlags;

    cmdPrefix =
      if msys2-env == null
      then ""
      else "wine";

    unpackPhase =
      ''
        tar xf ${src} --strip-components=1
      '';
    configurePhase =
      ''
        cd llvm
      ''
      + optionalString (msys2-env != null) ''
        export WINEDEBUG=-all
        export WINEPATH=Z:${msys2-env}/clang64/bin
      ''
      + ''
        export HOME=`mktemp -d`
      ''
      + extraConfig
      + ''
        mkdir build
        cd build
        ${cmdPrefix} cmake -G "Ninja" .. $cmakeFlags
      '';
    buildPhase =
      ''
        ${cmdPrefix} ninja -j $NIX_BUILD_CORES
      ''
      + (lib.strings.concatStrings (map (tool: "${cmdPrefix} ninja -j $NIX_BUILD_CORES " + tool + "\n") llvmTools));
    installPhase =
      ''
        ${cmdPrefix} ninja install
      ''
      + (lib.strings.concatStrings (map (tool: "cp bin/" + tool + "${exe_suffix} $out/bin\n") llvmTools));
    dontFixup = true;
  };
  llvm-tools-irrt =
    runCommand "llvm-tools-irrt" {}
    ''
      mkdir -p $out/bin
      ln -s ${llvm}/bin/clang${exe_suffix} $out/bin/clang-irrt${exe_suffix}
      ln -s ${llvm}/bin/llvm-as${exe_suffix} $out/bin/llvm-as-irrt${exe_suffix}
    '';
  clang = wrapCCWith rec {
    cc = stdenv.mkDerivation {
      name = "clang-nac3";

      dontUnpack = true;
      installPhase = ''
        mkdir -p $out/lib
        mkdir -p $out/bin
        cp -r ${llvm}/bin $out
        cp -r ${llvm}/lib $out
      '';
      passthru.isClang = true;
    };
  };
  compiler-rt = stdenv.mkDerivation {
    name = "compiler-rt";
    dontUnpack = true;

    installPhase = ''
      cp -r ${llvm}/lib/clang/${builtins.elemAt (lib.strings.splitString "." llvm.version) 0}/lib $out
    '';
  };
}
