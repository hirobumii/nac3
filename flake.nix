{
  description = "The third-generation ARTIQ compiler";

  inputs.nixpkgs.url = github:NixOS/nixpkgs/nixos-unstable;

  outputs = {
    self,
    nixpkgs,
  }: let
    pkgs = import nixpkgs {system = "x86_64-linux";};
    pkgs32 = import nixpkgs {system = "i686-linux";};
  in rec {
    packages.x86_64-linux = rec {
      llvm-nac3 = pkgs.callPackage ./nix/llvm {};
      llvm-tools-irrt =
        pkgs.runCommandNoCC "llvm-tools-irrt" {}
        ''
          mkdir -p $out/bin
          ln -s ${pkgs.llvmPackages_16.clang-unwrapped}/bin/clang $out/bin/clang-irrt
          ln -s ${pkgs.llvmPackages_16.llvm.out}/bin/llvm-as $out/bin/llvm-as-irrt
        '';
      demo-linalg-stub = pkgs.rustPlatform.buildRustPackage {
        name = "demo-linalg-stub";
        src = ./nac3standalone/demo/linalg;
        cargoLock = {
          lockFile = ./nac3standalone/demo/linalg/Cargo.lock;
        };
        doCheck = false;
      };
      demo-linalg-stub32 = pkgs32.rustPlatform.buildRustPackage {
        name = "demo-linalg-stub32";
        src = ./nac3standalone/demo/linalg;
        cargoLock = {
          lockFile = ./nac3standalone/demo/linalg/Cargo.lock;
        };
        doCheck = false;
      };
      nac3artiq = pkgs.python3Packages.toPythonModule (
        pkgs.rustPlatform.buildRustPackage rec {
          name = "nac3artiq";
          outputs = ["out" "runkernel" "standalone"];
          src = self;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          passthru.cargoLock = cargoLock;
          nativeBuildInputs = [pkgs.python3 (pkgs.wrapClangMulti pkgs.llvmPackages_16.clang) llvm-tools-irrt pkgs.llvmPackages_16.llvm.out pkgs.llvmPackages_16.bintools llvm-nac3];
          buildInputs = [pkgs.python3 llvm-nac3 pkgs.stdenv.cc.cc.lib];
          checkInputs = [(pkgs.python3.withPackages (ps: [ps.numpy ps.scipy]))];
          checkPhase = ''
            echo "Checking nac3standalone demos..."
            pushd nac3standalone/demo
            patchShebangs .
            export DEMO_LINALG_STUB=${demo-linalg-stub}/lib/liblinalg.a
            export DEMO_LINALG_STUB32=${demo-linalg-stub32}/lib/liblinalg.a
            ./check_demos.sh -i686
            popd
            echo "Running Cargo tests..."
            cargoCheckHook
          '';
          installPhase = ''
            PYTHON_SITEPACKAGES=$out/${pkgs.python3Packages.python.sitePackages}
            mkdir -p $PYTHON_SITEPACKAGES
            cp target/x86_64-unknown-linux-gnu/release/libnac3artiq.so $PYTHON_SITEPACKAGES/nac3artiq.so

            mkdir -p $runkernel/bin
            cp target/x86_64-unknown-linux-gnu/release/runkernel $runkernel/bin

            mkdir -p $standalone/bin
            cp target/x86_64-unknown-linux-gnu/release/nac3standalone $standalone/bin
          '';
        }
      );
      python3-mimalloc =
        pkgs.python3
        // rec {
          withMimalloc = pkgs.python3.buildEnv.override {makeWrapperArgs = ["--set LD_PRELOAD ${pkgs.mimalloc}/lib/libmimalloc.so"];};
          withPackages = f: let packages = f pkgs.python3.pkgs; in withMimalloc.override {extraLibs = packages;};
        };

      # LLVM PGO support
      llvm-nac3-instrumented = pkgs.callPackage ./nix/llvm {
        stdenv = pkgs.llvmPackages_16.stdenv;
        extraCmakeFlags = ["-DLLVM_BUILD_INSTRUMENTED=IR"];
      };
      nac3artiq-instrumented = pkgs.python3Packages.toPythonModule (
        pkgs.rustPlatform.buildRustPackage {
          name = "nac3artiq-instrumented";
          src = self;
          inherit (nac3artiq) cargoLock;
          nativeBuildInputs = [pkgs.python3 packages.x86_64-linux.llvm-tools-irrt pkgs.llvmPackages_16.bintools llvm-nac3-instrumented];
          buildInputs = [pkgs.python3 llvm-nac3-instrumented];
          cargoBuildFlags = ["--package" "nac3artiq" "--features" "init-llvm-profile"];
          doCheck = false;
          configurePhase = ''
            export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-L${pkgs.llvmPackages_16.compiler-rt}/lib/linux -C link-arg=-lclang_rt.profile-x86_64"
          '';
          installPhase = ''
            TARGET_DIR=$out/${pkgs.python3Packages.python.sitePackages}
            mkdir -p $TARGET_DIR
            cp target/x86_64-unknown-linux-gnu/release/libnac3artiq.so $TARGET_DIR/nac3artiq.so
          '';
        }
      );
      nac3artiq-profile = pkgs.stdenvNoCC.mkDerivation {
        name = "nac3artiq-profile";
        srcs = [
          (pkgs.fetchFromGitHub {
            owner = "m-labs";
            repo = "sipyco";
            rev = "96fcefbea490a9b42c862393860d2e586b05d744";
            sha256 = "sha256-DkcgZ0K6lsxzBWc31GTyufuSOpcorVv5OsZLHphHBtg=";
          })
          (pkgs.fetchFromGitHub {
            owner = "m-labs";
            repo = "artiq";
            rev = "79ccd2bd7bb4c42442ce76ce3463f2010eedd151";
            sha256 = "sha256-wYdhSB3nKd970Ux35rKiW4DYFdwntAsoeCnxdUXM/Ig=";
          })
        ];
        buildInputs = [
          (python3-mimalloc.withPackages (ps: [ps.numpy ps.scipy ps.jsonschema ps.lmdb ps.platformdirs nac3artiq-instrumented]))
          pkgs.llvmPackages_16.llvm.out
          pkgs.llvmPackages_16.bintools
        ];
        phases = ["buildPhase" "installPhase"];
        buildPhase = ''
          srcs=($srcs)
          sipyco=''${srcs[0]}
          artiq=''${srcs[1]}
          export PYTHONPATH=$sipyco:$artiq
          python -m artiq.frontend.artiq_ddb_template $artiq/artiq/examples/nac3devices/master.json -s 1 $artiq/artiq/examples/nac3devices/satellite.json > device_db.py
          cp $artiq/artiq/examples/nac3devices/nac3devices.py .
          python -m artiq.frontend.artiq_compile nac3devices.py
        '';
        installPhase = ''
          mkdir $out
          llvm-profdata merge -o $out/llvm.profdata /build/llvm/build/profiles/*
        '';
      };
      llvm-nac3-pgo = pkgs.callPackage ./nix/llvm {
        stdenv = pkgs.llvmPackages_16.stdenv;
        extraCmakeFlags = ["-DLLVM_PROFDATA_FILE=${nac3artiq-profile}/llvm.profdata"];
      };
      nac3artiq-pgo = pkgs.python3Packages.toPythonModule (
        pkgs.rustPlatform.buildRustPackage {
          name = "nac3artiq-pgo";
          src = self;
          inherit (nac3artiq) cargoLock;
          nativeBuildInputs = [pkgs.python3 packages.x86_64-linux.llvm-tools-irrt pkgs.llvmPackages_16.bintools llvm-nac3-pgo];
          buildInputs = [pkgs.python3 llvm-nac3-pgo];
          cargoBuildFlags = ["--package" "nac3artiq"];
          cargoTestFlags = ["--package" "nac3ast" "--package" "nac3parser" "--package" "nac3core" "--package" "nac3artiq"];
          installPhase = ''
            TARGET_DIR=$out/${pkgs.python3Packages.python.sitePackages}
            mkdir -p $TARGET_DIR
            cp target/x86_64-unknown-linux-gnu/release/libnac3artiq.so $TARGET_DIR/nac3artiq.so
          '';
        }
      );
    };

    packages.x86_64-w64-mingw32 = import ./nix/windows {inherit pkgs;};

    formatter.x86_64-linux = pkgs.alejandra;

    devShells.x86_64-linux.default = pkgs.mkShell {
      name = "nac3-dev-shell";
      buildInputs = with pkgs; [
        # build dependencies
        packages.x86_64-linux.llvm-nac3
        (pkgs.wrapClangMulti llvmPackages_16.clang)
        llvmPackages_16.llvm.out
        llvmPackages_16.bintools # for running nac3standalone demos
        packages.x86_64-linux.llvm-tools-irrt
        cargo
        rustc
        # runtime dependencies
        lld_16 # for running kernels on the host
        (packages.x86_64-linux.python3-mimalloc.withPackages (ps: [ps.numpy ps.scipy]))
        # development tools
        cargo-insta
        clippy
        pre-commit
        rustfmt
      ];
      shellHook = ''
        export DEMO_LINALG_STUB=${packages.x86_64-linux.demo-linalg-stub}/lib/liblinalg.a
        export DEMO_LINALG_STUB32=${packages.x86_64-linux.demo-linalg-stub32}/lib/liblinalg.a
      '';
    };
    devShells.x86_64-linux.msys2 = pkgs.mkShell {
      name = "nac3-dev-shell-msys2";
      buildInputs = with pkgs; [
        curl
        pacman
        fakeroot
        packages.x86_64-w64-mingw32.wine-msys2
      ];
    };

    hydraJobs = {
      inherit (packages.x86_64-linux) llvm-nac3 nac3artiq nac3artiq-pgo;
      llvm-nac3-msys2 = packages.x86_64-w64-mingw32.llvm-nac3;
      nac3artiq-msys2 = packages.x86_64-w64-mingw32.nac3artiq;
      nac3artiq-msys2-pkg = packages.x86_64-w64-mingw32.nac3artiq-pkg;
    };
  };

  nixConfig = {
    extra-trusted-public-keys = "nixbld.m-labs.hk-1:5aSRVA5b320xbNvu30tqxVPXpld73bhtOeH6uAjRyHc=";
    extra-substituters = "https://nixbld.m-labs.hk";
  };
}
