{pkgs}: [
  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-21.1.7-1-any.pkg.tar.zst";
    sha256 = "1hzynqksgrfv1b4478h7bw9wh4xh4l01665rxqiwpbjba8jwgnrd";
    name = "mingw-w64-clang-x86_64-libunwind-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-21.1.7-1-any.pkg.tar.zst";
    sha256 = "1ycxcxl3fpz8dx282xj07zb8k8i2mlpvrszf0zlfp4daacngn22r";
    name = "mingw-w64-clang-x86_64-libc++-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.5.2-1-any.pkg.tar.zst";
    sha256 = "02lc36mk43vi6lg4gb4dkyigk56fkqdk7b3ycapmih1w7kfyqq2r";
    name = "mingw-w64-clang-x86_64-libffi-3.5.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libiconv-1.18-1-any.pkg.tar.zst";
    sha256 = "0vn5xgx9jjg66f8r9ylm9220qdbjdkffykfl6nwj14zv9y7xh4nj";
    name = "mingw-w64-clang-x86_64-libiconv-1.18-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
    sha256 = "06i9xjsskf4ddb2ph4h31md5c7imj9mzjhd4lc4q44j8dmpc1w5p";
    name = "mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.15.1-2-any.pkg.tar.zst";
    sha256 = "02dyqdsz35dz0wi0a4w7mq9c3n45gs0w5n2k0gp7fd19p2v6hk9r";
    name = "mingw-w64-clang-x86_64-libxml2-2.15.1-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
    sha256 = "1hrx54k2s3dcs8fhwdwms5amr4gjid1d20b2b4302xyjg9yyvpxl";
    name = "mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-21.1.7-1-any.pkg.tar.zst";
    sha256 = "0xbzdk0za7i9fakzpi3aayxvwckyzd0l56f004kf166h967xvnby";
    name = "mingw-w64-clang-x86_64-llvm-libs-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-21.1.7-1-any.pkg.tar.zst";
    sha256 = "0ga7az9mmvkx9j7r7hnz2i8rpvm046w3mcza4k0w2rj3yfbzw4v2";
    name = "mingw-w64-clang-x86_64-clang-libs-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-21.1.7-1-any.pkg.tar.zst";
    sha256 = "0s5gg0bnbzcbvl6nl67klqs0i1xfvaz1wng0fi8nlmvn14jq8g90";
    name = "mingw-w64-clang-x86_64-compiler-rt-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-tools-21.1.7-1-any.pkg.tar.zst";
    sha256 = "1sih93h2f02vi400r21i81wgi1jw2aws93zlj7fw7j9ha7vhdkpn";
    name = "mingw-w64-clang-x86_64-llvm-tools-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-13.0.0.r338.gc1c6b0ff3-1-any.pkg.tar.zst";
    sha256 = "0x3zb6kxa2kws7gpi3rkdq609ljskml15r20m9ls0v2ivharj7as";
    name = "mingw-w64-clang-x86_64-headers-git-13.0.0.r338.gc1c6b0ff3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-13.0.0.r338.gc1c6b0ff3-1-any.pkg.tar.zst";
    sha256 = "0g7crgx1kbcs188kqbal0mmxa2xm8m22hrlig825k4444bbvmymq";
    name = "mingw-w64-clang-x86_64-crt-git-13.0.0.r338.gc1c6b0ff3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-21.1.7-1-any.pkg.tar.zst";
    sha256 = "0m6py7ai6f4454g9w0vlyvnzkgc2fd6k3yc1xnrdnkcb1g423q2q";
    name = "mingw-w64-clang-x86_64-lld-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-13.0.0.r338.gc1c6b0ff3-1-any.pkg.tar.zst";
    sha256 = "0rlyfirizj04q7kia3r1pz913ysjhwzrzbn380gy2cjsn87fyrr8";
    name = "mingw-w64-clang-x86_64-libwinpthread-13.0.0.r338.gc1c6b0ff3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-13.0.0.r338.gc1c6b0ff3-1-any.pkg.tar.zst";
    sha256 = "0ir92p6q6f0vl5dmd8vnz82f97p5lczm0k174bbf9r5fx7dnz73c";
    name = "mingw-w64-clang-x86_64-winpthreads-13.0.0.r338.gc1c6b0ff3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-21.1.7-1-any.pkg.tar.zst";
    sha256 = "0kiiqjvzaccv0wsy1k7s5xlbyvg4kgfbgzxf5mjdd26n45j766jr";
    name = "mingw-w64-clang-x86_64-clang-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-http-parser-2.9.4-2-any.pkg.tar.zst";
    sha256 = "0bmnpq7cqihspyma4xxg3rsz4z8wqc294pi7wfy2vxn26m82rfy5";
    name = "mingw-w64-clang-x86_64-http-parser-2.9.4-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.6.0-1-any.pkg.tar.zst";
    sha256 = "001ss0mzn618idcrwkgxlv7zc21j10dha46l8hxsxwgacrijv0b3";
    name = "mingw-w64-clang-x86_64-openssl-3.6.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.1-1-any.pkg.tar.zst";
    sha256 = "01l23cn5brficjzba7ldscqkdvk4rdcvvdyybd90qr2hqzligmhn";
    name = "mingw-w64-clang-x86_64-libssh2-1.11.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-bzip2-1.0.8-3-any.pkg.tar.zst";
    sha256 = "1n8zf2kk1xj7wiszp6mjchy1yzpalddbj0cj17qm625ags2vzflm";
    name = "mingw-w64-clang-x86_64-bzip2-1.0.8-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-wineditline-2.208-1-any.pkg.tar.zst";
    sha256 = "0x9d1ax81p0k1863z0jdhqg7454hb0svvil1f4aiqy1vlhqshfl5";
    name = "mingw-w64-clang-x86_64-wineditline-2.208-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pcre2-10.47-1-any.pkg.tar.zst";
    sha256 = "0ghwnhlb47fc36zc71rrxmkm10caip942fjkaym2rgvlgdn8q2zn";
    name = "mingw-w64-clang-x86_64-pcre2-10.47-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libgit2-1.9.2-1-any.pkg.tar.zst";
    sha256 = "1p3ay09vmgd8yi60bdrix7vad97f2cdmvm23ww8rkx7z6jwz5mcc";
    name = "mingw-w64-clang-x86_64-libgit2-1.9.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
    sha256 = "17ha468qavwin800cc3b7c3xdggwk2gakasfxg7jdx7616d99l0n";
    name = "mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-readline-8.3.001-1-any.pkg.tar.zst";
    sha256 = "182j0v3vppp04wg84n2qp4zakpby92a3jah9m0jy780pbfvncqyc";
    name = "mingw-w64-clang-x86_64-readline-8.3.001-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.51.1-1-any.pkg.tar.zst";
    sha256 = "1gnvh4nc4n9nxr02j6zpfq45v3k0j3sw16r28v9skvmfdk6lsnzw";
    name = "mingw-w64-clang-x86_64-sqlite3-3.51.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.91.1-1-any.pkg.tar.zst";
    sha256 = "1s8dgdls2pk1shs1jladd7a69k6skxprpm2jyfbf5w2yl3cvvrzv";
    name = "mingw-w64-clang-x86_64-rust-1.91.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
    sha256 = "0phhwkcqp30dsyj5vr6w99sgm1jfm5rzg0w5x5mv9md4x7lm9lmh";
    name = "mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.34.5-1-any.pkg.tar.zst";
    sha256 = "0r5kinyb90l0fr74zz7a5sn9qa4mgmz4j33azgyz1xx7zmchq5mh";
    name = "mingw-w64-clang-x86_64-c-ares-1.34.5-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-brotli-1.2.0-1-any.pkg.tar.zst";
    sha256 = "0pa83a86xkqd6r80hnz0ldafhxcgf13vvb0vlnv8cnzchfbvpgr9";
    name = "mingw-w64-clang-x86_64-brotli-1.2.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gettext-runtime-0.26-2-any.pkg.tar.zst";
    sha256 = "1y925rmy9d1c1ywi16gbq194jfnzxi5zrb67ssfq03hrcyq1d20i";
    name = "mingw-w64-clang-x86_64-gettext-runtime-0.26-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunistring-1.3-1-any.pkg.tar.zst";
    sha256 = "1zg58qbfybyqzcj0dalb13l48f9jsras318h02rka65r7wi0pdcg";
    name = "mingw-w64-clang-x86_64-libunistring-1.3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libidn2-2.3.8-4-any.pkg.tar.zst";
    sha256 = "138kbfy6v20jija1rw1rqrjf8bcxivfqkyi6xv276blc0sbngdci";
    name = "mingw-w64-clang-x86_64-libidn2-2.3.8-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libpsl-0.21.5-3-any.pkg.tar.zst";
    sha256 = "0hb7wgdliic3d7fa0cvr5pj946pmwfc0apmyb0yfb5d0hc1afwsc";
    name = "mingw-w64-clang-x86_64-libpsl-0.21.5-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtasn1-4.20.0-1-any.pkg.tar.zst";
    sha256 = "0hv0xayhzhpwp8bdcs2r4xdvimk6266h68ki8abnii0pqiwfi86r";
    name = "mingw-w64-clang-x86_64-libtasn1-4.20.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-p11-kit-0.25.10-1-any.pkg.tar.zst";
    sha256 = "1vwlyv5c52hmc2madb8rw0s30msncymawh2vgaiy648x7dsfbwi6";
    name = "mingw-w64-clang-x86_64-p11-kit-0.25.10-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20250419-1-any.pkg.tar.zst";
    sha256 = "1nhnqyh5wxlzg60nh1i9fcadgwsbln0vkgff1y8cn3fkxap15lxb";
    name = "mingw-w64-clang-x86_64-ca-certificates-20250419-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.68.0-1-any.pkg.tar.zst";
    sha256 = "06qpymb5nq7pk0ildvnbw7ykl18kk44z0wa2azg4a6fipvn6ignp";
    name = "mingw-w64-clang-x86_64-nghttp2-1.68.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gmp-6.3.0-2-any.pkg.tar.zst";
    sha256 = "03j72zks06pbwqbwsmv84f1441c333gy0k7d1yxzds95diyggwk9";
    name = "mingw-w64-clang-x86_64-gmp-6.3.0-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nettle-3.10.2-1-any.pkg.tar.zst";
    sha256 = "13gfzr5dfxcr5m050g3irwqk88lald25l0gybz130wnajp6sk0g8";
    name = "mingw-w64-clang-x86_64-nettle-3.10.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gnutls-3.8.11-1-any.pkg.tar.zst";
    sha256 = "0wvkri2nh5d7iin8j1a06w1jmxq7cdjla1bdgcv6w1xnyblk35qg";
    name = "mingw-w64-clang-x86_64-gnutls-3.8.11-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ngtcp2-1.18.0-1-any.pkg.tar.zst";
    sha256 = "0p2qkir65ldllyvsz4l45z4idhzic0x46j49qy961ww038h15hyv";
    name = "mingw-w64-clang-x86_64-ngtcp2-1.18.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.13.1-1-any.pkg.tar.zst";
    sha256 = "1w786dk7qxij4j88ag86dijlhzwfqfm2gj9il27w99rjhfrhdvix";
    name = "mingw-w64-clang-x86_64-nghttp3-1.13.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.17.0-1-any.pkg.tar.zst";
    sha256 = "0rmbwzgb3yf4n0rk5w8rj77xlkzsy0c5g6phf1gfv94mf4z8id16";
    name = "mingw-w64-clang-x86_64-curl-8.17.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.7.3-1-any.pkg.tar.zst";
    sha256 = "04sp1xlv9hx39zhh5i557frmifgkk1lwx333iwbn33f061kmfz3h";
    name = "mingw-w64-clang-x86_64-expat-2.7.3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-jsoncpp-1.9.6-3-any.pkg.tar.zst";
    sha256 = "1ipilhiza17vz5dhgi61l80w2klw9f21w6jbyhi9wmfd6nxqv13c";
    name = "mingw-w64-clang-x86_64-jsoncpp-1.9.6-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libb2-0.98.1-3-any.pkg.tar.zst";
    sha256 = "1qn2xlvv1xc3qyfqar3fdmg7mqfsvaa1x68y9jwbvar7xbqnjb3g";
    name = "mingw-w64-clang-x86_64-libb2-0.98.1-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lz4-1.10.0-1-any.pkg.tar.zst";
    sha256 = "0kznnw9z9zqxkmn8qbypm2rpsfaapbgls1ks3zzpfnfjz9cpw8py";
    name = "mingw-w64-clang-x86_64-lz4-1.10.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtre-0.9.0-2-any.pkg.tar.zst";
    sha256 = "0iswbz4d3nrgq6wsdy9887rqwfvz8bh1lj7vfb89s0j54nrjg2yf";
    name = "mingw-w64-clang-x86_64-libtre-0.9.0-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libsystre-1.0.2-2-any.pkg.tar.zst";
    sha256 = "16dzv6czgsr5mk1s9ay9syxg4vbqv2smnz07k5zfr94585i5wdca";
    name = "mingw-w64-clang-x86_64-libsystre-1.0.2-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.8.1-2-any.pkg.tar.zst";
    sha256 = "1mms5mk0qqp5hlwxkcrcjr8dv77jvnay6527z4886n1a99mlsniv";
    name = "mingw-w64-clang-x86_64-xz-5.8.1-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.8.4-1-any.pkg.tar.zst";
    sha256 = "1almyckmzykcvpyfb00ra9fv548ygy2a4gayk15h3v667zi11mks";
    name = "mingw-w64-clang-x86_64-libarchive-3.8.4-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.51.0-1-any.pkg.tar.zst";
    sha256 = "1bh8fjsqb15abvcxay2pyk741jb88ygjgqah0s9fcbc7wfj4p1br";
    name = "mingw-w64-clang-x86_64-libuv-1.51.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ninja-1.13.2-1-any.pkg.tar.zst";
    sha256 = "0rcsli4w20ajx4kmcjccdwi9293g5xqhbxknq7wlvhhz4ia83dnv";
    name = "mingw-w64-clang-x86_64-ninja-1.13.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pkgconf-1~2.5.1-1-any.pkg.tar.zst";
    sha256 = "1srsggda5rkwsif82jrfxskvb10ix2nw38xk0nc7jpnf0ab529bb";
    name = "mingw-w64-clang-x86_64-pkgconf-12.5.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.6-1-any.pkg.tar.zst";
    sha256 = "0pjhi9p926zbbv9h3p83np3yjpdajpf1s1fid7x9hc9vc3x499sf";
    name = "mingw-w64-clang-x86_64-rhash-1.4.6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-4.2.0-1-any.pkg.tar.zst";
    sha256 = "0ly9m0gmnr1qsghwnl8xkzxzzlin6b1m2mi1735prh29whdmk4av";
    name = "mingw-w64-clang-x86_64-cmake-4.2.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-4.0.1-1-any.pkg.tar.zst";
    sha256 = "17nk5cj3rfsi82kay359kalrajf0qmi70innvr6h36g5d57mnwf4";
    name = "mingw-w64-clang-x86_64-mpdecimal-4.0.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.5.20250927-2-any.pkg.tar.zst";
    sha256 = "1wa1wmriflar3hvmpvfvapc6ggpawldl3m5vqp1a1i49r6f34rpi";
    name = "mingw-w64-clang-x86_64-ncurses-6.5.20250927-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tcl-8.6.17-1-any.pkg.tar.zst";
    sha256 = "1q66vdgdxzwyf4ai4ml76jnscffj99pjrq22y4fpacxhsj62i2l1";
    name = "mingw-w64-clang-x86_64-tcl-8.6.17-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tk-8.6.17-2-any.pkg.tar.zst";
    sha256 = "1jwhxsqr5d7dfqbip16spppfwnqxqcayx2hycnk8dsbxhw28cldn";
    name = "mingw-w64-clang-x86_64-tk-8.6.17-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2025b-2-any.pkg.tar.zst";
    sha256 = "04r54rzpf0vlpbidywshwb6l2lyql6wqbwcgplpicpkxgbcilpsa";
    name = "mingw-w64-clang-x86_64-tzdata-2025b-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.12.12-1-any.pkg.tar.zst";
    sha256 = "1kl6a9f2irsxkxgah5yh29givpjw44j5ppqqg5zs0cxpnydjww94";
    name = "mingw-w64-clang-x86_64-python-3.12.12-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-21.1.7-1-any.pkg.tar.zst";
    sha256 = "01dvws2kgsf0sky6122xi8j8pw7zf27j64a02bg028gqf778xsyg";
    name = "mingw-w64-clang-x86_64-llvm-openmp-21.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.30-2-any.pkg.tar.zst";
    sha256 = "01x3gj0yk6x48nc3f0rg0gzl246sv2cidvrjhvy9ic8x1wl69xmd";
    name = "mingw-w64-clang-x86_64-openblas-0.3.30-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.3.5-1-any.pkg.tar.zst";
    sha256 = "057spcjy2czy3vr2z14np8y1yki066x2xjabwwv87nk3j0ml9s6z";
    name = "mingw-w64-clang-x86_64-python-numpy-2.3.5-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-80.9.0-2-any.pkg.tar.zst";
    sha256 = "0vq9fw6dm8cvf835hd5aqyjq6bfcdcwmj7k1xdm5v09bdbr2cf7b";
    name = "mingw-w64-clang-x86_64-python-setuptools-80.9.0-2-any.pkg.tar.zst";
  })
]
