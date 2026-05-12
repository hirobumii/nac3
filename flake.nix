{
  description = "The third-generation ARTIQ compiler";

  inputs.nixpkgs.url = github:NixOS/nixpkgs/nixos-unstable;

  outputs = {
    self,
    nixpkgs,
  }: let
    pkgs = import nixpkgs {system = "x86_64-linux";};
    pkgs32 = import nixpkgs {system = "i686-linux";};
    llvm-nac3 = pkgs.callPackage ./nix/llvm {
      enableProjects = ["clang" "compiler-rt"];
      llvmTools = ["llvm-config" "llvm-as" "llvm-profdata"];
    };
  in rec {
    packages.x86_64-linux = rec {
      inherit (llvm-nac3) llvm llvm-tools-irrt clang compiler-rt;
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
          nativeBuildInputs = [pkgs.python3 (pkgs.wrapClangMulti clang) llvm llvm-tools-irrt];
          buildInputs = [pkgs.python3 llvm pkgs.stdenv.cc.cc.lib pkgs.zlib pkgs.ncurses];
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
      llvm-nac3-instrumented =
        (pkgs.callPackage ./nix/llvm {
          extraCmakeFlags = [
            "-DLLVM_BUILD_INSTRUMENTED=IR"
            "-DLLVM_BUILD_RUNTIME=No"
            "-DCMAKE_C_COMPILER=${clang}/bin/clang"
            "-DCMAKE_CXX_COMPILER=${clang}/bin/clang++"
            "-DLLVM_NATIVE_TOOL_DIR=${llvm}/bin"
          ];
        }).llvm;
      nac3artiq-instrumented = pkgs.python3Packages.toPythonModule (
        pkgs.rustPlatform.buildRustPackage {
          name = "nac3artiq-instrumented";
          src = self;
          inherit (nac3artiq) cargoLock;
          nativeBuildInputs = [pkgs.python3 llvm-tools-irrt llvm-nac3-instrumented];
          buildInputs = [pkgs.python3 llvm-nac3-instrumented pkgs.zlib pkgs.ncurses];
          cargoBuildFlags = ["--package" "nac3artiq" "--features" "init-llvm-profile"];
          doCheck = false;
          configurePhase = ''
            export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-L${compiler-rt}/x86_64-unknown-linux-gnu -C link-arg=-lclang_rt.profile"
            export LLVM_SYS_221_PREFIX=${llvm-nac3-instrumented}
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
          (pkgs.fetchgit {
            url = "https://git.m-labs.hk/M-Labs/sipyco.git";
            rev = "ab3d738ee302a2a37304e8ee59bb23e30bfb81ae";
            hash = "sha256-85pe9Y56HhmcdYnyaiHZr56eJesoeTHLoQKcknX/Scw=";
          })
          (pkgs.fetchgit {
            url = "https://git.m-labs.hk/M-Labs/artiq.git";
            rev = "0e69d9e688a90a16cc1eb3e0e8fed836db8864f1";
            hash = "sha256-31/A33oCsFpMMSc+9YO9YjwQtbxN0HXTVypgD4nfW0k=";
          })
        ];
        buildInputs = [
          (python3-mimalloc.withPackages (ps: [ps.numpy ps.scipy ps.jsonschema ps.lmdb ps.platformdirs nac3artiq-instrumented]))
          llvm
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
      llvm-nac3-pgo =
        (pkgs.callPackage ./nix/llvm {
          extraCmakeFlags = [
            "-DLLVM_PROFDATA_FILE=${nac3artiq-profile}/llvm.profdata"
            "-DCMAKE_C_COMPILER=${clang}/bin/clang"
            "-DCMAKE_CXX_COMPILER=${clang}/bin/clang++"
            "-DLLVM_NATIVE_TOOL_DIR=${llvm}/bin"
          ];
        }).llvm;
      nac3artiq-pgo = pkgs.python3Packages.toPythonModule (
        pkgs.rustPlatform.buildRustPackage {
          name = "nac3artiq-pgo";
          src = self;
          inherit (nac3artiq) cargoLock;
          nativeBuildInputs = [pkgs.python3 llvm-tools-irrt llvm-nac3-pgo];
          buildInputs = [pkgs.python3 llvm-nac3-pgo pkgs.zlib pkgs.ncurses];
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
        (pkgs.wrapClangMulti packages.x86_64-linux.clang)
        packages.x86_64-linux.llvm
        packages.x86_64-linux.llvm-tools-irrt
        zlib
        ncurses
        cargo
        rustc
        # runtime dependencies
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
      inherit (packages.x86_64-linux) nac3artiq nac3artiq-profile;
      llvm-nac3 = packages.x86_64-linux.llvm;
      llvm-nac3-msys2 = packages.x86_64-w64-mingw32.llvm;
      nac3artiq-msys2 = packages.x86_64-w64-mingw32.nac3artiq;
      nac3artiq-msys2-pkg = packages.x86_64-w64-mingw32.nac3artiq-pkg;
    };
  };

  nixConfig = {
    extra-trusted-public-keys = "nixbld.m-labs.hk-1:5aSRVA5b320xbNvu30tqxVPXpld73bhtOeH6uAjRyHc=";
    extra-substituters = "https://nixbld.m-labs.hk";
  };
}
