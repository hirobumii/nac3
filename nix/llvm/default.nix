{
  lib,
  stdenv,
  pkgsBuildBuild,
  fetchurl,
  fetchpatch,
  cmake,
  python3,
  libbfd,
  ncurses,
  zlib,
  which,
  debugVersion ? false,
  enableSharedLibraries ? false,
  extraCmakeFlags ? [],
}: let
  inherit (lib) optional optionals optionalString;
  sources = import ../llvm/sources.nix {inherit fetchurl;};
in
  stdenv.mkDerivation rec {
    pname = "llvm";
    inherit (sources) version;

    unpackPhase = ''
      mkdir llvm
      tar xf ${sources.llvm} -C llvm --strip-components=1
      tar xf ${sources.cmake} -C llvm/cmake --strip-components=2
      mkdir cmake
      ln -s $PWD/llvm/cmake cmake/Modules
      sourceRoot=$PWD/llvm
    '';

    outputs = ["out" "lib" "dev" "python"];

    nativeBuildInputs = [cmake python3];

    buildInputs = [];

    propagatedBuildInputs = [ncurses zlib];

    checkInputs = [which];

    patches = [
      ./gnu-install-dirs.patch
    ];

    postPatch = ''
      # FileSystem permissions tests fail with various special bits
      substituteInPlace unittests/Support/CMakeLists.txt \
        --replace-fail "Path.cpp" ""
      rm unittests/Support/Path.cpp
      substituteInPlace unittests/IR/CMakeLists.txt \
        --replace-fail "PassBuilderCallbacksTest.cpp" ""
      rm unittests/IR/PassBuilderCallbacksTest.cpp
      rm test/tools/llvm-objcopy/ELF/mirror-permissions-unix.test
      patchShebangs test/BugPoint/compile-custom.ll.py
    '';

    # hacky fix: created binaries need to be run before installation
    preBuild = ''
      mkdir -p $out/
      ln -sv $PWD/lib $out
    '';

    # E.g. mesa.drivers use the build-id as a cache key (see #93946):
    LDFLAGS = optionalString enableSharedLibraries "-Wl,--build-id=sha1";

    cmakeFlags = with stdenv;
      [
        "-DLLVM_INSTALL_PACKAGE_DIR=${placeholder "dev"}/lib/cmake/llvm"
        "-DCMAKE_BUILD_TYPE=${
          if debugVersion
          then "Debug"
          else "Release"
        }"
        "-DLLVM_INCLUDE_TESTS=OFF"
        "-DLLVM_HOST_TRIPLE=${stdenv.hostPlatform.config}"
        "-DLLVM_DEFAULT_TARGET_TRIPLE=${stdenv.hostPlatform.config}"
        "-DLLVM_ENABLE_UNWIND_TABLES=OFF"
        "-DLLVM_ENABLE_THREADS=ON"
        "-DLLVM_INCLUDE_BENCHMARKS=OFF"
        "-DLLVM_BUILD_TOOLS=OFF"
        "-DLLVM_TARGETS_TO_BUILD=X86;ARM;RISCV"
      ]
      ++ optionals enableSharedLibraries [
        "-DLLVM_LINK_LLVM_DYLIB=ON"
      ]
      ++ optionals (!isDarwin && !stdenv.targetPlatform.isMinGW) [
        "-DLLVM_BINUTILS_INCDIR=${libbfd.dev}/include"
      ]
      ++ optionals (stdenv.hostPlatform != stdenv.buildPlatform) [
        "-DCMAKE_CROSSCOMPILING=True"
        (
          let
            nativeCC = pkgsBuildBuild.targetPackages.stdenv.cc;
            nativeBintools = nativeCC.bintools.bintools;
            nativeToolchainFlags = [
              "-DCMAKE_C_COMPILER=${nativeCC}/bin/${nativeCC.targetPrefix}cc"
              "-DCMAKE_CXX_COMPILER=${nativeCC}/bin/${nativeCC.targetPrefix}c++"
              "-DCMAKE_AR=${nativeBintools}/bin/${nativeBintools.targetPrefix}ar"
              "-DCMAKE_STRIP=${nativeBintools}/bin/${nativeBintools.targetPrefix}strip"
              "-DCMAKE_RANLIB=${nativeBintools}/bin/${nativeBintools.targetPrefix}ranlib"
            ];
          in "-DCROSS_TOOLCHAIN_FLAGS_NATIVE:list=${lib.concatStringsSep ";" nativeToolchainFlags}"
        )
      ]
      ++ extraCmakeFlags;

    postBuild = ''
      make llvm-config
      rm -fR $out
    '';

    preCheck = ''
      export LD_LIBRARY_PATH=$LD_LIBRARY_PATH''${LD_LIBRARY_PATH:+:}$PWD/lib
    '';

    postInstall =
      ''
        cp bin/llvm-config $out/bin
        mkdir -p $python/share
        mv $out/share/opt-viewer $python/share/opt-viewer
        moveToOutput "bin/llvm-config*" "$dev"
        substituteInPlace "$dev/lib/cmake/llvm/LLVMExports-${
          if debugVersion
          then "debug"
          else "release"
        }.cmake" \
          --replace "\''${_IMPORT_PREFIX}/lib/lib" "$lib/lib/lib" \
          --replace "$out/bin/llvm-config" "$dev/bin/llvm-config"
        substituteInPlace "$dev/lib/cmake/llvm/LLVMConfig.cmake" \
          --replace 'set(LLVM_BINARY_DIR "''${LLVM_INSTALL_PREFIX}")' 'set(LLVM_BINARY_DIR "''${LLVM_INSTALL_PREFIX}'"$lib"'")'
      ''
      + optionalString (stdenv.buildPlatform != stdenv.hostPlatform) ''
        cp NATIVE/bin/llvm-config $dev/bin/llvm-config-native
      '';

    doCheck = false; # the ABI change breaks RISC-V FP tests

    checkTarget = "check-all";

    requiredSystemFeatures = ["big-parallel"];
    meta = {
      homepage = "https://llvm.org/";
      description = "A collection of modular and reusable compiler and toolchain technologies";
      longDescription = ''
        The LLVM Project is a collection of modular and reusable compiler and
        toolchain technologies. Despite its name, LLVM has little to do with
        traditional virtual machines. The name "LLVM" itself is not an acronym; it
        is the full name of the project.
        LLVM began as a research project at the University of Illinois, with the
        goal of providing a modern, SSA-based compilation strategy capable of
        supporting both static and dynamic compilation of arbitrary programming
        languages. Since then, LLVM has grown to be an umbrella project consisting
        of a number of subprojects, many of which are being used in production by
        a wide variety of commercial and open source projects as well as being
        widely used in academic research. Code in the LLVM project is licensed
        under the "Apache 2.0 License with LLVM exceptions".
      '';
    };
  }
