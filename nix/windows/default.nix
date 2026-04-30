{pkgs}: let
  msys2-env = pkgs.stdenvNoCC.mkDerivation rec {
    name = "msys2-env";
    srcs = import ./msys2_packages.nix {inherit pkgs;};
    buildInputs = [pkgs.gnutar pkgs.zstd];
    phases = ["installPhase"];
    installPhase = pkgs.lib.strings.concatStringsSep "\n" (["mkdir $out"] ++ (map (p: "tar xvf ${p} -C $out") srcs));
  };
  silenceFontconfig =
    # silence flood of "Fontconfig error: Cannot load default config file: No such file: (null)"
    ''
      export FONTCONFIG_PATH=$HOME/fonts
      mkdir $FONTCONFIG_PATH
      cat > $FONTCONFIG_PATH/fonts.conf << EOF
      <fontconfig>
      </fontconfig>
      EOF
    '';
  pyo3-mingw-config = pkgs.writeTextFile {
    name = "pyo3-mingw-config";
    text = ''
      implementation=CPython
      version=3.14
      shared=true
      abi3=false
      lib_name=python3.14
      lib_dir=${msys2-env}/clang64/lib
      pointer_width=64
      build_flags=WITH_THREAD
      suppress_build_script_link_lines=false
    '';
  };
  sources = import ../llvm/sources.nix {inherit (pkgs) fetchurl;};
  llvm-nac3 = pkgs.callPackage ../llvm {
    stdenv = pkgs.stdenvNoCC;
    inherit msys2-env;
    enableProjects = ["clang"];
    llvmTools = ["llvm-config" "llvm-as"];
    extraConfig = silenceFontconfig;
  };
in rec {
  inherit (llvm-nac3) llvm llvm-tools-irrt compiler-rt;
  nac3artiq = pkgs.rustPlatform.buildRustPackage {
    name = "nac3artiq-msys2";
    src = ../../.;
    cargoLock = {
      lockFile = ../../Cargo.lock;
    };
    nativeBuildInputs = [pkgs.wineWow64Packages.stable];
    buildPhase = ''
      export HOME=`mktemp -d`
      export WINEDEBUG=-all
      export WINEPATH=Z:${msys2-env}/clang64/bin\;Z:${llvm}/bin\;Z:${llvm-tools-irrt}/bin
      ${silenceFontconfig}
      export PYO3_CONFIG_FILE=Z:${pyo3-mingw-config}
      export CC=clang
      export LLVM_SYS_191_PREFIX=Z:${llvm}
      wine cargo build --release -p nac3artiq
    '';
    installPhase = ''
      mkdir $out $out/nix-support
      cp target/release/nac3artiq.dll $out/nac3artiq.pyd
      echo file binary-dist $out/nac3artiq.pyd >> $out/nix-support/hydra-build-products
    '';
    doCheck = false; # https://git.m-labs.hk/M-Labs/nac3/issues/358
    checkPhase = ''
      wine cargo test --release
    '';
    dontFixup = true;
  };
  nac3artiq-pkg = pkgs.stdenvNoCC.mkDerivation {
    name = "nac3artiq-msys2-pkg";
    nativeBuildInputs = [pkgs.pacman pkgs.fakeroot pkgs.libarchive pkgs.zstd];
    src = nac3artiq;
    phases = ["buildPhase" "installPhase"];
    buildPhase = ''
      ln -s ${./PKGBUILD} PKGBUILD
      ln -s $src/nac3artiq.pyd nac3artiq.pyd
      makepkg --config ${./makepkg.conf} --nodeps
    '';
    installPhase = ''
      mkdir $out $out/nix-support
      cp *.pkg.tar.zst $out
      echo file msys2 $out/*.pkg.tar.zst >> $out/nix-support/hydra-build-products
    '';
  };
  wine-msys2 =
    pkgs.writeShellScriptBin "wine-msys2"
    ''
      export WINEDEBUG=-all
      export WINEPATH=Z:${msys2-env}/clang64/bin\;Z:${llvm}/bin\;Z:${llvm-tools-irrt}/bin
      export PYO3_CONFIG_FILE=Z:${pyo3-mingw-config}
      exec ${pkgs.wineWow64Packages.stable}/bin/wine cmd
    '';
  wine-msys2-build =
    pkgs.writeShellScriptBin "wine-msys2-build"
    ''
      export HOME=`mktemp -d`
      export WINEDEBUG=-all
      export WINEPATH=Z:${msys2-env}/clang64/bin
      ${silenceFontconfig}
      exec ${pkgs.wineWow64Packages.stable}/bin/wine $@
    '';
}
