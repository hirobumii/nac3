{pkgs}: [
  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-21.1.8-1-any.pkg.tar.zst";
    sha256 = "0iym44s85ywcbzr30plammrkw65f0w0sas5p7jh1lj1b1ljy0av3";
    name = "mingw-w64-clang-x86_64-libunwind-21.1.8-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-21.1.8-1-any.pkg.tar.zst";
    sha256 = "1l2qhpfqf1lbzyidd4118839pbjwr2m63h1j6jdpdzgap2qnxvj4";
    name = "mingw-w64-clang-x86_64-libc++-21.1.8-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.2-2-any.pkg.tar.zst";
    sha256 = "0phbb2wz5l01ahkwwf5xm0v7bncp6h5db6dqh970sk7j5cpxpn4n";
    name = "mingw-w64-clang-x86_64-zlib-1.3.2-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.15.2-1-any.pkg.tar.zst";
    sha256 = "1j0hkgz6dglp9hsj867l3n5b8n79z99k4ini0b2bf3migk847fkz";
    name = "mingw-w64-clang-x86_64-libxml2-2.15.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
    sha256 = "1hrx54k2s3dcs8fhwdwms5amr4gjid1d20b2b4302xyjg9yyvpxl";
    name = "mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-21.1.8-4-any.pkg.tar.zst";
    sha256 = "04wfw2d8f8ymd5p6ma1ry1ixhq8ay87bch38msp0xafziqx0a18z";
    name = "mingw-w64-clang-x86_64-llvm-libs-21.1.8-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-21.1.8-4-any.pkg.tar.zst";
    sha256 = "0v7m4s12n1iff3xp48hglrym0533izxi6zi5j5sqv8lfq15l092r";
    name = "mingw-w64-clang-x86_64-clang-libs-21.1.8-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-21.1.8-4-any.pkg.tar.zst";
    sha256 = "0ps2msgzhh11jzrd9gazqpsxharqjqc2h3xr3vs11k6vfv9zgvqv";
    name = "mingw-w64-clang-x86_64-compiler-rt-21.1.8-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-tools-21.1.8-4-any.pkg.tar.zst";
    sha256 = "1hw6mb2ajazqhr04kfyn47prpfwxwjs74slizvhi4y8h6jy2xmag";
    name = "mingw-w64-clang-x86_64-llvm-tools-21.1.8-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-13.0.0.r560.g3197fc7d6-1-any.pkg.tar.zst";
    sha256 = "0qy0rk8ap3773ybpqscnzjz22xdsi9xzy6ypbbl89fik4k7jir7p";
    name = "mingw-w64-clang-x86_64-headers-13.0.0.r560.g3197fc7d6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-13.0.0.r560.g3197fc7d6-1-any.pkg.tar.zst";
    sha256 = "0xxpslka2wpdvrxmwkv0yvv98xdrbp9fah4cqcpi00ipf36y2kiv";
    name = "mingw-w64-clang-x86_64-crt-13.0.0.r560.g3197fc7d6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-21.1.8-4-any.pkg.tar.zst";
    sha256 = "0mdhadzmk81zzzag2qvnxgx1rqzv48c625h1fy2czapal2nsb10f";
    name = "mingw-w64-clang-x86_64-lld-21.1.8-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-13.0.0.r560.g3197fc7d6-1-any.pkg.tar.zst";
    sha256 = "1faddlrvy40kll9bc0k31j6bdvqb93z1d5kn3nnj3g8pihq4qnc7";
    name = "mingw-w64-clang-x86_64-libwinpthread-13.0.0.r560.g3197fc7d6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-13.0.0.r560.g3197fc7d6-1-any.pkg.tar.zst";
    sha256 = "11cch1jhy4rv9vxg7j0bwxhp83jhc52169r6pvjlhkh4lbrvwiwi";
    name = "mingw-w64-clang-x86_64-winpthreads-13.0.0.r560.g3197fc7d6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-21.1.8-4-any.pkg.tar.zst";
    sha256 = "19fvj9pmdfr8zpz45r9ca32mq25592863rdk8rcsvbpb4dflzw9x";
    name = "mingw-w64-clang-x86_64-clang-21.1.8-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-http-parser-2.9.4-3-any.pkg.tar.zst";
    sha256 = "0na0kji862wr80xym65rr8m9qcyp2424acirr2gn696lflrq3arw";
    name = "mingw-w64-clang-x86_64-http-parser-2.9.4-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.6.1-3-any.pkg.tar.zst";
    sha256 = "1pq166imxcrlq27fq0rb56jk9b2dlayky04ad8hx4kyrdps5rihg";
    name = "mingw-w64-clang-x86_64-openssl-3.6.1-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.1-2-any.pkg.tar.zst";
    sha256 = "066808p483nhivnd02czs9aqd24arcggvgxwg79hg2kax4wdmav5";
    name = "mingw-w64-clang-x86_64-libssh2-1.11.1-2-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libgit2-1.9.2-2-any.pkg.tar.zst";
    sha256 = "031f7a6gyvc8gfmac83fd3xnhw20vjbigbkd8haj0d9mvxgs8c7z";
    name = "mingw-w64-clang-x86_64-libgit2-1.9.2-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
    sha256 = "17ha468qavwin800cc3b7c3xdggwk2gakasfxg7jdx7616d99l0n";
    name = "mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-readline-8.3.003-1-any.pkg.tar.zst";
    sha256 = "0hyzwyhc08786vwzaappiija02p94pqfb1f8aafywqza1rc4h21f";
    name = "mingw-w64-clang-x86_64-readline-8.3.003-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.51.2-1-any.pkg.tar.zst";
    sha256 = "1vvgfg84b6fds03l08m5q6c96pb82lczdakmf243a6llid1sa43a";
    name = "mingw-w64-clang-x86_64-sqlite3-3.51.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.94.0-1-any.pkg.tar.zst";
    sha256 = "0piwiknmfvnilbw8bjfbngy9d2hh8c0lb4n72lrvs363br1as8rs";
    name = "mingw-w64-clang-x86_64-rust-1.94.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
    sha256 = "0phhwkcqp30dsyj5vr6w99sgm1jfm5rzg0w5x5mv9md4x7lm9lmh";
    name = "mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.34.6-1-any.pkg.tar.zst";
    sha256 = "14r4g4ya6xqikcijfl5wlyvfpcv37pr3317dbvxzliacn6xlxrvw";
    name = "mingw-w64-clang-x86_64-c-ares-1.34.6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-brotli-1.2.0-1-any.pkg.tar.zst";
    sha256 = "0pa83a86xkqd6r80hnz0ldafhxcgf13vvb0vlnv8cnzchfbvpgr9";
    name = "mingw-w64-clang-x86_64-brotli-1.2.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gettext-runtime-1.0-1-any.pkg.tar.zst";
    sha256 = "145vd12i2i85km425pkqlqfnjl1w794wkfl9haf9sv414x3sc8kg";
    name = "mingw-w64-clang-x86_64-gettext-runtime-1.0-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtasn1-4.21.0-1-any.pkg.tar.zst";
    sha256 = "03mj97ml74nd8qh63h6lf4xaqvc06z2a0ypd9zsfjwcckm3ajkln";
    name = "mingw-w64-clang-x86_64-libtasn1-4.21.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-p11-kit-0.26.2-1-any.pkg.tar.zst";
    sha256 = "02s8yszqm80m2nfnb5vil42adahifr71my8d8npz1p7i03wbi4ar";
    name = "mingw-w64-clang-x86_64-p11-kit-0.26.2-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gnutls-3.8.12-1-any.pkg.tar.zst";
    sha256 = "091iskkh1vq8j3sy7dibxj6pq9nck0yl1n02jsakk2mmn6d15xaj";
    name = "mingw-w64-clang-x86_64-gnutls-3.8.12-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ngtcp2-1.21.0-1-any.pkg.tar.zst";
    sha256 = "1sic34nrjk5mm438qw59zg2kd03bfbb9axkfpybs8ibk540kmgv0";
    name = "mingw-w64-clang-x86_64-ngtcp2-1.21.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.15.0-1-any.pkg.tar.zst";
    sha256 = "0vdi14dvkpx196ib6jir3lglgrjn0kz4hdjf8myjmw1czljw9cfn";
    name = "mingw-w64-clang-x86_64-nghttp3-1.15.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.18.0-4-any.pkg.tar.zst";
    sha256 = "07d1ssnna6h2zp3ghsvyw286sixlrd4i646lw7nh059g5x2wv5sz";
    name = "mingw-w64-clang-x86_64-curl-8.18.0-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.7.4-1-any.pkg.tar.zst";
    sha256 = "157lvqvd0f9998i422bm2401liqcgvbll9q4q5hh5l5zd1cbw1km";
    name = "mingw-w64-clang-x86_64-expat-2.7.4-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.8.2-1-any.pkg.tar.zst";
    sha256 = "1vpbnavcahij13rwq48m4wj5ny2g2vb7r2mw5mfx6k3pfk4kk32p";
    name = "mingw-w64-clang-x86_64-xz-5.8.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.8.5-2-any.pkg.tar.zst";
    sha256 = "1b8ihbccfhj4by58mr7m4gfb6j77wi0gd0wwiws5cf62rssnnhjl";
    name = "mingw-w64-clang-x86_64-libarchive-3.8.5-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.52.0-1-any.pkg.tar.zst";
    sha256 = "13barq1m94sv143gvqla57dmmcyc7f55mg2nzgh7lwxv9sk4qygi";
    name = "mingw-w64-clang-x86_64-libuv-1.52.0-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-4.2.3-1-any.pkg.tar.zst";
    sha256 = "0kfjh2aqyxpn2h9frjvg8avj0ggjmfwxxivw7jz09n7256x4hs45";
    name = "mingw-w64-clang-x86_64-cmake-4.2.3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-4.0.1-1-any.pkg.tar.zst";
    sha256 = "17nk5cj3rfsi82kay359kalrajf0qmi70innvr6h36g5d57mnwf4";
    name = "mingw-w64-clang-x86_64-mpdecimal-4.0.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.6-2-any.pkg.tar.zst";
    sha256 = "07ya1l098nr50n83kx52ydvgsnism9rcj15c038xz0dsixssvmcj";
    name = "mingw-w64-clang-x86_64-ncurses-6.6-2-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2026a-1-any.pkg.tar.zst";
    sha256 = "0jh6ri6k4a0d8arg045fgvsw9d76ah3n1wmrnaxpq6akvpc4av2q";
    name = "mingw-w64-clang-x86_64-tzdata-2026a-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.14.3-1-any.pkg.tar.zst";
    sha256 = "1a7bj0dcq6mkshlf6v85n0n4xfyswk206cfniiv284vbl7yzv3va";
    name = "mingw-w64-clang-x86_64-python-3.14.3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-21.1.8-2-any.pkg.tar.zst";
    sha256 = "0svn7khdj36f0nvnnzpy96476c6nm8cg92hkvnyl9rdacrv9a52g";
    name = "mingw-w64-clang-x86_64-llvm-openmp-21.1.8-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.31-1-any.pkg.tar.zst";
    sha256 = "176wad25z75w514pp45v1khn6ya81cnapm8skad61ilw28xqv3ra";
    name = "mingw-w64-clang-x86_64-openblas-0.3.31-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.4.1-2-any.pkg.tar.zst";
    sha256 = "0860glzjn6g4xnfwgbhciidysn1p50rksm3n2dv7lvjnndnkn1l3";
    name = "mingw-w64-clang-x86_64-python-numpy-2.4.1-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-81.0.0-1-any.pkg.tar.zst";
    sha256 = "1f7bwgnnqn6p9i2kj7ldl2wz1d4wx163iskrlsfpjfr84anh38lg";
    name = "mingw-w64-clang-x86_64-python-setuptools-81.0.0-1-any.pkg.tar.zst";
  })
]
