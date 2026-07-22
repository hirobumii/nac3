{pkgs}: [
  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-22.1.8-1-any.pkg.tar.zst";
    sha256 = "1zfmvcw8y1mql6765fmbvn5q9mc62siy8wfz0rdh81iknhx5h9ql";
    name = "mingw-w64-clang-x86_64-libunwind-22.1.8-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-22.1.8-1-any.pkg.tar.zst";
    sha256 = "1vz4zgjx2lzmycpbc6syf3k67g9zvbw5m4ga0snbd09gsswx22yy";
    name = "mingw-w64-clang-x86_64-libc++-22.1.8-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.7.1-1-any.pkg.tar.zst";
    sha256 = "026czhx6vrnvzjhwp3prykz2mfvps5b9jwdd3jslbnjcbrr0xgvg";
    name = "mingw-w64-clang-x86_64-libffi-3.7.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libiconv-1.19-1-any.pkg.tar.zst";
    sha256 = "1v09sgng6n2m7jh9qlvj1z6s1185qr0an719zgzwvvi342gsri16";
    name = "mingw-w64-clang-x86_64-libiconv-1.19-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.2-2-any.pkg.tar.zst";
    sha256 = "0phbb2wz5l01ahkwwf5xm0v7bncp6h5db6dqh970sk7j5cpxpn4n";
    name = "mingw-w64-clang-x86_64-zlib-1.3.2-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.15.3-1-any.pkg.tar.zst";
    sha256 = "0fjdsg05l8z93i6bkj3x6vm5s48q0dkgz8z2mrqcvfhn3laf4w96";
    name = "mingw-w64-clang-x86_64-libxml2-2.15.3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.7-2-any.pkg.tar.zst";
    sha256 = "0zfqzzq74ba6hv48ahp1xlx9n3s52vm1wgrywdpvgxzlz3fw8x9d";
    name = "mingw-w64-clang-x86_64-zstd-1.5.7-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-22.1.8-2-any.pkg.tar.zst";
    sha256 = "0613l40rqclvicmk9maihm5r9bd3yspyq7fgr28cnjg2sqbppn33";
    name = "mingw-w64-clang-x86_64-llvm-libs-22.1.8-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-22.1.8-2-any.pkg.tar.zst";
    sha256 = "16hl48h29flh6hn97rkhyhws7bqdcnn5abgbicnk4rk2fnyhcl9k";
    name = "mingw-w64-clang-x86_64-clang-libs-22.1.8-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-22.1.8-2-any.pkg.tar.zst";
    sha256 = "11pm82a5bqpgldrywf5l0fazl74f58qyfa95k06x3giq96qpa1l4";
    name = "mingw-w64-clang-x86_64-compiler-rt-22.1.8-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-tools-22.1.8-2-any.pkg.tar.zst";
    sha256 = "0isf2x62gbd9rd6i0b34p0y4mvzgi9x6shb1fkmnr8x572fzhxn1";
    name = "mingw-w64-clang-x86_64-llvm-tools-22.1.8-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-14.0.0.r190.g96fb1bff7-1-any.pkg.tar.zst";
    sha256 = "01b5h7q697h2ab81kk1b70vbjs9s78dr0kpbzph7y4wncc68jszy";
    name = "mingw-w64-clang-x86_64-headers-14.0.0.r190.g96fb1bff7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-14.0.0.r190.g96fb1bff7-1-any.pkg.tar.zst";
    sha256 = "1d02d8hgyx3pgkphg1zl37xbz92z57nqy3b2yxfj4h484bgp5qc3";
    name = "mingw-w64-clang-x86_64-crt-14.0.0.r190.g96fb1bff7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-22.1.8-2-any.pkg.tar.zst";
    sha256 = "0wdmnzc3my8swdy6hc5w745ina16li40ry88146f4a8950g22ryv";
    name = "mingw-w64-clang-x86_64-lld-22.1.8-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-14.0.0.r190.g96fb1bff7-1-any.pkg.tar.zst";
    sha256 = "0xabnrncj95qdkmw7pia1jcprzccn6b85xsqdgjsrib83y8if7m9";
    name = "mingw-w64-clang-x86_64-libwinpthread-14.0.0.r190.g96fb1bff7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-14.0.0.r190.g96fb1bff7-1-any.pkg.tar.zst";
    sha256 = "03cxpnsa96fm3d1gklq9nb9d92jknpqblx7h6i2vafn8gkpl447b";
    name = "mingw-w64-clang-x86_64-winpthreads-14.0.0.r190.g96fb1bff7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-22.1.8-2-any.pkg.tar.zst";
    sha256 = "0p8sp3h300whfx1lgghd4y29nqjy4qsc7iv2ag2kvrdds4wrrfaf";
    name = "mingw-w64-clang-x86_64-clang-22.1.8-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-http-parser-2.9.4-3-any.pkg.tar.zst";
    sha256 = "0na0kji862wr80xym65rr8m9qcyp2424acirr2gn696lflrq3arw";
    name = "mingw-w64-clang-x86_64-http-parser-2.9.4-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.6.3-1-any.pkg.tar.zst";
    sha256 = "0js063wz4av1wwvzxndl3la29kqbdm5519wlhsfjc8ddilkl34gd";
    name = "mingw-w64-clang-x86_64-openssl-3.6.3-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libgit2-1.9.6-1-any.pkg.tar.zst";
    sha256 = "0102kzkp7xzrznvazgia61nm4nqn7yncabmw0z1ra1cmailiaxcb";
    name = "mingw-w64-clang-x86_64-libgit2-1.9.6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tcl-8.6.18-1-any.pkg.tar.zst";
    sha256 = "0jr6y59gy6ar5m0gjisymp0nisx5cand2zp1zlm9q203lybhqz00";
    name = "mingw-w64-clang-x86_64-tcl-8.6.18-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.53.3-1-any.pkg.tar.zst";
    sha256 = "0vl8dxawvhkx0w0nkws9phnf10gkgss9p4wc74nf25d1q3170ncr";
    name = "mingw-w64-clang-x86_64-sqlite3-3.53.3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.97.0-1-any.pkg.tar.zst";
    sha256 = "1cqdwniv9dmx2myg3285w6vcvvy8z1rac72xhnn955dmw82zir7k";
    name = "mingw-w64-clang-x86_64-rust-1.97.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
    sha256 = "0phhwkcqp30dsyj5vr6w99sgm1jfm5rzg0w5x5mv9md4x7lm9lmh";
    name = "mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.34.8-1-any.pkg.tar.zst";
    sha256 = "18p04ffwwrqsl5hsfsfpf1rjm6vb60d45i1zxi9sjql2vrjz1xg9";
    name = "mingw-w64-clang-x86_64-c-ares-1.34.8-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunistring-1.4.2-1-any.pkg.tar.zst";
    sha256 = "1cc4ahp7klsz2pgd5xn0hhf5hnrkx778jxilyq4aavyvs4dp6z2y";
    name = "mingw-w64-clang-x86_64-libunistring-1.4.2-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-p11-kit-0.26.4-1-any.pkg.tar.zst";
    sha256 = "1ngy6wjc7cs4gmfyz3k7hjr3ygqkncj2axgi901v5qv9znq57gx1";
    name = "mingw-w64-clang-x86_64-p11-kit-0.26.4-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20250419-1-any.pkg.tar.zst";
    sha256 = "1nhnqyh5wxlzg60nh1i9fcadgwsbln0vkgff1y8cn3fkxap15lxb";
    name = "mingw-w64-clang-x86_64-ca-certificates-20250419-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.69.0-1-any.pkg.tar.zst";
    sha256 = "0vjyqxmqg1j2lvps6wsalimvcpiyy6a5n7wj4gl4klg3a4zvjzv7";
    name = "mingw-w64-clang-x86_64-nghttp2-1.69.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gmp-6.3.0-2-any.pkg.tar.zst";
    sha256 = "03j72zks06pbwqbwsmv84f1441c333gy0k7d1yxzds95diyggwk9";
    name = "mingw-w64-clang-x86_64-gmp-6.3.0-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nettle-4.0-1-any.pkg.tar.zst";
    sha256 = "0k5njjjmb1nzr259dxg1k81p09ma3w5bmqn8qmf2x30rwb40cv32";
    name = "mingw-w64-clang-x86_64-nettle-4.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gnutls-3.8.13-3-any.pkg.tar.zst";
    sha256 = "1hl745vgcrr20pklrsh2ll4f7pc799pdzznvvnphwv3pzny7b4z4";
    name = "mingw-w64-clang-x86_64-gnutls-3.8.13-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ngtcp2-1.24.0-1-any.pkg.tar.zst";
    sha256 = "102dxj3xl2qqci15ryh1cp90agmw3jv98nmklwdjzr4b9736h14q";
    name = "mingw-w64-clang-x86_64-ngtcp2-1.24.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.17.0-1-any.pkg.tar.zst";
    sha256 = "1nf2901z7r8swza558y9hvg2i6lcygq9l4fc9g5m8rz28xl5fh1q";
    name = "mingw-w64-clang-x86_64-nghttp3-1.17.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.21.0-2-any.pkg.tar.zst";
    sha256 = "1663r18mjflw5svclcyfagcgsjyydhj360r5dyn4y9wgljvyjj6h";
    name = "mingw-w64-clang-x86_64-curl-8.21.0-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.8.2-1-any.pkg.tar.zst";
    sha256 = "1lcp93mw8qi3v6wz047lnf43xv5im7rpb5d8sw62g20z3q5h4mc0";
    name = "mingw-w64-clang-x86_64-expat-2.8.2-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.8.3-1-any.pkg.tar.zst";
    sha256 = "0yv1viwvaxnj4hrgpb2bxav1f57015zcpcfs9amqkp5mv8xc7cfw";
    name = "mingw-w64-clang-x86_64-xz-5.8.3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.8.8-2-any.pkg.tar.zst";
    sha256 = "14a0f4xi4l4xx8dbfsmhjmc2nabqa0zpz58hg2cqvaykhandhbpx";
    name = "mingw-w64-clang-x86_64-libarchive-3.8.8-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.52.1-1-any.pkg.tar.zst";
    sha256 = "06yy72qm7c5ci40bsz4axl762vjzvj7aa690zha4cwisbrsy6sb3";
    name = "mingw-w64-clang-x86_64-libuv-1.52.1-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ninja-1.13.2-1-any.pkg.tar.zst";
    sha256 = "0rcsli4w20ajx4kmcjccdwi9293g5xqhbxknq7wlvhhz4ia83dnv";
    name = "mingw-w64-clang-x86_64-ninja-1.13.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pkgconf-1~3.0.3-1-any.pkg.tar.zst";
    sha256 = "0357hwcfdrr03xqsx27k9bngkk665vyki26qn6bcs4lxl6bxv6i6";
    name = "mingw-w64-clang-x86_64-pkgconf-13.0.3-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.6-1-any.pkg.tar.zst";
    sha256 = "0pjhi9p926zbbv9h3p83np3yjpdajpf1s1fid7x9hc9vc3x499sf";
    name = "mingw-w64-clang-x86_64-rhash-1.4.6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-4.4.0-1-any.pkg.tar.zst";
    sha256 = "0bqwvd8s852jylxkm212hmb45f8insd3h9kgghv3mzg3j28i4s0a";
    name = "mingw-w64-clang-x86_64-cmake-4.4.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-4.0.1-3-any.pkg.tar.zst";
    sha256 = "1jasdycw273fw5pa3bmj67r2rfffqf2nw6nsw2qc761y4zx4cfgf";
    name = "mingw-w64-clang-x86_64-mpdecimal-4.0.1-3-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.6-4-any.pkg.tar.zst";
    sha256 = "0m90y87zqhkgynwj73pk2c96drdk56r70jsgn5zask7bn9hlrd6b";
    name = "mingw-w64-clang-x86_64-ncurses-6.6-4-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tk-8.6.18-1-any.pkg.tar.zst";
    sha256 = "192f4hx928svv9qfn7zagnp6gx1v26lknhzq9ix07wdbryyb2pf8";
    name = "mingw-w64-clang-x86_64-tk-8.6.18-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2026c-1-any.pkg.tar.zst";
    sha256 = "083xsxy0wfy6q6dd1kcc911iwxfg15k5dm9f7vp3vs785ry9gw1n";
    name = "mingw-w64-clang-x86_64-tzdata-2026c-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.14.6-1-any.pkg.tar.zst";
    sha256 = "04am3sgj0sk4hk7wyb1v80iplrvhlcd0l20pi39q0wj1lfl5j00a";
    name = "mingw-w64-clang-x86_64-python-3.14.6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-22.1.8-1-any.pkg.tar.zst";
    sha256 = "0mja0lrxi8dqcx2dz64lb1xsy04cfj1yzrakp57n3rwaa28cdlay";
    name = "mingw-w64-clang-x86_64-llvm-openmp-22.1.8-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.33-3-any.pkg.tar.zst";
    sha256 = "0a48hc54rgnbjb4r6pp8c23mbpwa5vhxavrmwr18f4ddh3fjr5mk";
    name = "mingw-w64-clang-x86_64-openblas-0.3.33-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.4.6-1-any.pkg.tar.zst";
    sha256 = "0mzy04nsigm6f0sbs93kj0vjxbmcqky0gnk5svzrhlrrimgi7p3f";
    name = "mingw-w64-clang-x86_64-python-numpy-2.4.6-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-83.0.0-2-any.pkg.tar.zst";
    sha256 = "1730jalqqcaj3syr277yy7d3057qfickj5lh0la2npgr3sf3w4sj";
    name = "mingw-w64-clang-x86_64-python-setuptools-83.0.0-2-any.pkg.tar.zst";
  })
]
