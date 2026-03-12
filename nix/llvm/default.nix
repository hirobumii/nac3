{
  lib,
  stdenv,
  wineWow64Packages,
  fetchurl,
  fetchpatch,
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
  sources = import ../llvm/sources.nix {
    inherit fetchurl;
  };
in rec {
  exe_suffix =
    if msys2-env == null
    then ""
    else ".exe";
  llvm = stdenv.mkDerivation rec {
    pname = "llvm-nac3";
    version = sources.version;
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
        "-DCMAKE_BUILD_TYPE=Release"
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
        mkdir llvm
        tar xf ${sources.llvm} -C llvm --strip-components=1
        tar xf ${sources.cmake} -C llvm/cmake --strip-components=2
      ''
      + (lib.strings.concatStrings (map (proj: "mkdir " + proj + "\ntar xf " + builtins.getAttr proj sources + " -C " + proj + " --strip-components=1\n") enableProjects))
      + ''
        mkdir cmake
        ln -s $PWD/llvm/cmake cmake/Modules
      '';
    patches =
      lib.lists.flatten (map (proj:
        if proj == "clang"
        then [
          # clang ignores all "compile-only options" if it only performs linkage.
          # "Include path options" are "compile-only options".
          # However, clang-16 does not identify options such as -nostdlibinc as "include path options".
          # Hence, clang-16 always emit unused arguments warning when only linking.
          #
          # TODO: Remove when updating llvm.
          (fetchpatch {
            url = "https://github.com/llvm/llvm-project/commit/5b77e752dcd073846b89559d6c0e1a7699e58615.patch";
            sha256 = "sha256-W81hy5EWlRIpqu7BEEem+EKPFgHn3rYychH3cnD5aDc=";
          })
        ]
        else [])
      enableProjects)
      ++ optionals (msys2-env == null) ([
          # Aggregate of 2 merged patches:
          # https://github.com/llvm/llvm-project/commit/7e44305041d96b064c197216b931ae3917a34ac1.patch
          # https://github.com/llvm/llvm-project/commit/7abf44069aec61eee147ca67a6333fc34583b524.patch
          # Both to appease gcc15.
          ./llvm-gcc15.patch
        ]
        ++ lib.lists.flatten (map (proj:
          if proj == "compiler-rt"
          then [
            ./compiler-rt-gcc15.patch # Ditto. But no commits on compiler-rt.
            (fetchpatch {
              url = "https://github.com/llvm/llvm-project/commit/59978b21ad9c65276ee8e14f26759691b8a65763.patch";
              sha256 = "sha256-JrGBvwVtAat/HwT1PCq2TXCDwx/dZOUB/ThNFVJ5pMg=";
            })
          ]
          else [])
        enableProjects));
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
      cp -r ${llvm}/lib/clang/${builtins.elemAt (lib.strings.splitString "." sources.version) 0}/lib $out
    '';
  };
}
