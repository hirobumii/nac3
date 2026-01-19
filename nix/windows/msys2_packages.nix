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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
    sha256 = "06i9xjsskf4ddb2ph4h31md5c7imj9mzjhd4lc4q44j8dmpc1w5p";
    name = "mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.15.1-3-any.pkg.tar.zst";
    sha256 = "1j34v0y696pbq7w83b2lkgxwwy3a704b9j1ldhfla2j270sbhh2z";
    name = "mingw-w64-clang-x86_64-libxml2-2.15.1-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
    sha256 = "1hrx54k2s3dcs8fhwdwms5amr4gjid1d20b2b4302xyjg9yyvpxl";
    name = "mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-21.1.8-3-any.pkg.tar.zst";
    sha256 = "054w3cmsn56lhijqxy2qm84hnwcajgb7dc357gyvz233qd0vrfxs";
    name = "mingw-w64-clang-x86_64-llvm-libs-21.1.8-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-21.1.8-3-any.pkg.tar.zst";
    sha256 = "0irhzwg741r31v73dhfvivgaim79928vwyh4xfvj84xw3hrsfz2h";
    name = "mingw-w64-clang-x86_64-clang-libs-21.1.8-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-21.1.8-3-any.pkg.tar.zst";
    sha256 = "0iv9lcacclidyijpp5by34gvcy6fkvjdhvskaiyz6x1b1pikcfiy";
    name = "mingw-w64-clang-x86_64-compiler-rt-21.1.8-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-tools-21.1.8-3-any.pkg.tar.zst";
    sha256 = "1792prr67q3iz3b9x14p650ayvi1292ywmici5xdsyrn4vc1fn57";
    name = "mingw-w64-clang-x86_64-llvm-tools-21.1.8-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-13.0.0.r453.gfd36ef357-1-any.pkg.tar.zst";
    sha256 = "0kb5yv43l1pnbvbf8s729mjv6jkqwzh4fhw7b0lbwqqs77373iyg";
    name = "mingw-w64-clang-x86_64-headers-13.0.0.r453.gfd36ef357-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-13.0.0.r453.gfd36ef357-1-any.pkg.tar.zst";
    sha256 = "02dcjx1m5pv50mclj647fgpw5bzr18nabziyc32b8ba8gjz61cnl";
    name = "mingw-w64-clang-x86_64-crt-13.0.0.r453.gfd36ef357-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-21.1.8-3-any.pkg.tar.zst";
    sha256 = "142s0n6vr4hcy6m7cr8fbcbm44nvxindvh5lwkg1d2nc1s0dbdvy";
    name = "mingw-w64-clang-x86_64-lld-21.1.8-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-13.0.0.r453.gfd36ef357-1-any.pkg.tar.zst";
    sha256 = "0rdc550dmmqf4sgsbz95zm1dmlj7plf632bz48vcd84mvny0p6s1";
    name = "mingw-w64-clang-x86_64-libwinpthread-13.0.0.r453.gfd36ef357-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-13.0.0.r453.gfd36ef357-1-any.pkg.tar.zst";
    sha256 = "1py0hmlbaifqvpvnrbwzn2xp1saqsvxn3sbfgdwmvkgljgz3ifli";
    name = "mingw-w64-clang-x86_64-winpthreads-13.0.0.r453.gfd36ef357-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-21.1.8-3-any.pkg.tar.zst";
    sha256 = "0ajswwv6rmscm1slb0m0m6g6igd7v7j1snrhqw7bsnvrhnvwf9jv";
    name = "mingw-w64-clang-x86_64-clang-21.1.8-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-http-parser-2.9.4-3-any.pkg.tar.zst";
    sha256 = "0na0kji862wr80xym65rr8m9qcyp2424acirr2gn696lflrq3arw";
    name = "mingw-w64-clang-x86_64-http-parser-2.9.4-3-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.92.0-1-any.pkg.tar.zst";
    sha256 = "00zhs6fcr8ipin2kmdpihnlc48kr3nnaq4pnz3m0rdvbild4jk6x";
    name = "mingw-w64-clang-x86_64-rust-1.92.0-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtasn1-4.21.0-1-any.pkg.tar.zst";
    sha256 = "03mj97ml74nd8qh63h6lf4xaqvc06z2a0ypd9zsfjwcckm3ajkln";
    name = "mingw-w64-clang-x86_64-libtasn1-4.21.0-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ngtcp2-1.19.0-1-any.pkg.tar.zst";
    sha256 = "1j97hlbmakg33v2dd3cfv0jb9xins1alc1av4ysyllzrfbrc807n";
    name = "mingw-w64-clang-x86_64-ngtcp2-1.19.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.14.0-1-any.pkg.tar.zst";
    sha256 = "04widfkq5js0fp1w52fvxx0h2s7fzyd6jskihi2gxmsmh7rq6bxg";
    name = "mingw-w64-clang-x86_64-nghttp3-1.14.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.18.0-1-any.pkg.tar.zst";
    sha256 = "1nbyinmn78hv4nn9ikkhsi3lsjlmb09q8nh9gw1g625j2f2xs3iz";
    name = "mingw-w64-clang-x86_64-curl-8.18.0-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.8.2-1-any.pkg.tar.zst";
    sha256 = "1vpbnavcahij13rwq48m4wj5ny2g2vb7r2mw5mfx6k3pfk4kk32p";
    name = "mingw-w64-clang-x86_64-xz-5.8.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.8.5-1-any.pkg.tar.zst";
    sha256 = "0r6ybbcnzqf8pcpf6wymfgpbm27ck0cpkmklali0y9fvqjvcijsa";
    name = "mingw-w64-clang-x86_64-libarchive-3.8.5-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-4.2.1-1-any.pkg.tar.zst";
    sha256 = "1qjs42kilam7gkry6wmxmi9z5cdsc76m8jbb8bg11ynxbm5v5zpx";
    name = "mingw-w64-clang-x86_64-cmake-4.2.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-4.0.1-1-any.pkg.tar.zst";
    sha256 = "17nk5cj3rfsi82kay359kalrajf0qmi70innvr6h36g5d57mnwf4";
    name = "mingw-w64-clang-x86_64-mpdecimal-4.0.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.6-1-any.pkg.tar.zst";
    sha256 = "0bbfmi014hccdzgvf9f7anh8spl2s1njrv363256vbl6vv01wz8b";
    name = "mingw-w64-clang-x86_64-ncurses-6.6-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2025c-1-any.pkg.tar.zst";
    sha256 = "14lrhna5mgqb19jdzi4xwjszz2wy9kcpwz0z8v6yjzcidkn9mc7w";
    name = "mingw-w64-clang-x86_64-tzdata-2025c-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.13.11-4-any.pkg.tar.zst";
    sha256 = "037a7k1z9vfmab7s4kd27vx4c7iwlz7xry68yl828ghyik5s3k9c";
    name = "mingw-w64-clang-x86_64-python-3.13.11-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-21.1.8-1-any.pkg.tar.zst";
    sha256 = "18vv4j9l3wnxvfixaszsfc5cdnnfx4b9a5ckhj3q7z9rz08cwhc4";
    name = "mingw-w64-clang-x86_64-llvm-openmp-21.1.8-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.31-1-any.pkg.tar.zst";
    sha256 = "176wad25z75w514pp45v1khn6ya81cnapm8skad61ilw28xqv3ra";
    name = "mingw-w64-clang-x86_64-openblas-0.3.31-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.4.1-1-any.pkg.tar.zst";
    sha256 = "0r72ir1sxihskrd6rncqbvy2h7a9yz1hcslc41wakip5s82jar34";
    name = "mingw-w64-clang-x86_64-python-numpy-2.4.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-80.9.0-6-any.pkg.tar.zst";
    sha256 = "01an804lybsjir3adippishm8sya6fbp9di5qdxwpwf52xg70zgc";
    name = "mingw-w64-clang-x86_64-python-setuptools-80.9.0-6-any.pkg.tar.zst";
  })
]
