{pkgs}: [
  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-22.1.7-1-any.pkg.tar.zst";
    sha256 = "1xmv7srnvy0j6c7lb0k65vj2q6yjjjn83m72ib3cb5nnxcc6jf21";
    name = "mingw-w64-clang-x86_64-libunwind-22.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-22.1.7-1-any.pkg.tar.zst";
    sha256 = "0fvlgwckkdc89jnqqq7jyhjl5gh71p6kggfp163f5403k40c5s3q";
    name = "mingw-w64-clang-x86_64-libc++-22.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.5.2-1-any.pkg.tar.zst";
    sha256 = "02lc36mk43vi6lg4gb4dkyigk56fkqdk7b3ycapmih1w7kfyqq2r";
    name = "mingw-w64-clang-x86_64-libffi-3.5.2-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-22.1.7-1-any.pkg.tar.zst";
    sha256 = "1kwz2y73shgq5lx5fs7fh12j6fs6xkdsl3inl16n5rj071wqwafh";
    name = "mingw-w64-clang-x86_64-llvm-libs-22.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-22.1.7-1-any.pkg.tar.zst";
    sha256 = "1h1kwbfzhc4m5lx4zlbrzgrabxxzv6s62qx9sw1dgyp6lj34nhwv";
    name = "mingw-w64-clang-x86_64-clang-libs-22.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-22.1.7-1-any.pkg.tar.zst";
    sha256 = "0qp0kblhdx9r426q9mhw7b4l6gr56hfpg851xvfgm6h6a070flsx";
    name = "mingw-w64-clang-x86_64-compiler-rt-22.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-tools-22.1.7-1-any.pkg.tar.zst";
    sha256 = "1dqwpjkvrfmiv82fqvggnsbrj5gyh2hypnwjvhv14f3d6p38izf1";
    name = "mingw-w64-clang-x86_64-llvm-tools-22.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-14.0.0.r59.g93753750c-1-any.pkg.tar.zst";
    sha256 = "07l5imniv4h1x3pjdwbjp0bp7ana5drg9gr85b3iqrmr3pmpqxav";
    name = "mingw-w64-clang-x86_64-headers-14.0.0.r59.g93753750c-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-14.0.0.r59.g93753750c-1-any.pkg.tar.zst";
    sha256 = "06m9n43wpjj2l3da6nqa6rqh8r0s5hbbbhl3w9ylx6djnibvzlg2";
    name = "mingw-w64-clang-x86_64-crt-14.0.0.r59.g93753750c-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-22.1.7-1-any.pkg.tar.zst";
    sha256 = "1dn6jik1z3h6w6l4fs2b7r6f96ig5s3s7vhd9ja717j377kl9irz";
    name = "mingw-w64-clang-x86_64-lld-22.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-14.0.0.r59.g93753750c-1-any.pkg.tar.zst";
    sha256 = "1kx5nsv15c747d88lc67ysa04mlk61wnw1ms4a8c88m2xxbxj8sk";
    name = "mingw-w64-clang-x86_64-libwinpthread-14.0.0.r59.g93753750c-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-14.0.0.r59.g93753750c-1-any.pkg.tar.zst";
    sha256 = "08g48sjbdi188a8ybf5an9aalzjrjsf4yrd81yqnr3wy15n7hldc";
    name = "mingw-w64-clang-x86_64-winpthreads-14.0.0.r59.g93753750c-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-22.1.7-1-any.pkg.tar.zst";
    sha256 = "12rvb1rqb1xikg5qlwl4n95g5wnjq8fnz6a50lvcmls7znd9wsnw";
    name = "mingw-w64-clang-x86_64-clang-22.1.7-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-http-parser-2.9.4-3-any.pkg.tar.zst";
    sha256 = "0na0kji862wr80xym65rr8m9qcyp2424acirr2gn696lflrq3arw";
    name = "mingw-w64-clang-x86_64-http-parser-2.9.4-3-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.6.2-2-any.pkg.tar.zst";
    sha256 = "16w7xb1f1g9wqfrah5020xrdh5hl30sv8hhd5gka4pvqlxpvljvp";
    name = "mingw-w64-clang-x86_64-openssl-3.6.2-2-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libgit2-1.9.4-1-any.pkg.tar.zst";
    sha256 = "1h4n0zkyidi3fdkqz2gv0kgp63s6nxjvlbq0cwiykkh5mvly3r82";
    name = "mingw-w64-clang-x86_64-libgit2-1.9.4-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tcl-8.6.18-1-any.pkg.tar.zst";
    sha256 = "0jr6y59gy6ar5m0gjisymp0nisx5cand2zp1zlm9q203lybhqz00";
    name = "mingw-w64-clang-x86_64-tcl-8.6.18-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.53.2-1-any.pkg.tar.zst";
    sha256 = "0gmvy4qgzlpxiz88ny3sda2ml3j7rvk3x6m4b2sm40gd07mmv58c";
    name = "mingw-w64-clang-x86_64-sqlite3-3.53.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.96.0-1-any.pkg.tar.zst";
    sha256 = "0knwbi7sdb2z02pnk0mg4bpcyq2rc8v4whhzlw6cx5cr97z8zrz4";
    name = "mingw-w64-clang-x86_64-rust-1.96.0-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nettle-3.10.2-1-any.pkg.tar.zst";
    sha256 = "13gfzr5dfxcr5m050g3irwqk88lald25l0gybz130wnajp6sk0g8";
    name = "mingw-w64-clang-x86_64-nettle-3.10.2-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gnutls-3.8.13-2-any.pkg.tar.zst";
    sha256 = "0jn7s7zbx414wl3rmfsxvm4k00w8j08kwrjxqiq8qxg0nqa65x8s";
    name = "mingw-w64-clang-x86_64-gnutls-3.8.13-2-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ngtcp2-1.23.0-1-any.pkg.tar.zst";
    sha256 = "06pwk7ijrk44zpdvx57dyrc2d6f35z4f1nkcklbmwrwp3kvdn2kp";
    name = "mingw-w64-clang-x86_64-ngtcp2-1.23.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.16.0-1-any.pkg.tar.zst";
    sha256 = "19f8d1dpkavlwbmq70gv6vr442r5zfwhj661ab2g05mp0972c9ll";
    name = "mingw-w64-clang-x86_64-nghttp3-1.16.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.20.0-1-any.pkg.tar.zst";
    sha256 = "10q91w5gcm4jhcshqs2jfb0kw2s082119kbzn4sfvykqgzz2d7fz";
    name = "mingw-w64-clang-x86_64-curl-8.20.0-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.8.1-1-any.pkg.tar.zst";
    sha256 = "19dfhipc8p5y2bz1zyzqy2cv488cc7n0085fxmqamlhrw0xkkvdd";
    name = "mingw-w64-clang-x86_64-expat-2.8.1-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.8.7-1-any.pkg.tar.zst";
    sha256 = "15h13700dq492rgs9h8cnksvv6w9hzpa3rdnch6hyhrxplph6sc3";
    name = "mingw-w64-clang-x86_64-libarchive-3.8.7-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-4.3.3-1-any.pkg.tar.zst";
    sha256 = "1598idv4n0slpihhyw0gv6xx49an5phdizfc9bxivkwji8hmqna3";
    name = "mingw-w64-clang-x86_64-cmake-4.3.3-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2026b-1-any.pkg.tar.zst";
    sha256 = "16mwcaix3vnlmwxln4is13fd4bw7hzgrkhgqqrl64l79kfqxnbqw";
    name = "mingw-w64-clang-x86_64-tzdata-2026b-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.14.5-1-any.pkg.tar.zst";
    sha256 = "0vm9d8dwxmckc30rf2mc0mhn40b8vh3y85v8qbyjyz3z1813szaz";
    name = "mingw-w64-clang-x86_64-python-3.14.5-1-any.pkg.tar.zst";
  })

  (pkgs.fetchurl {
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-22.1.7-1-any.pkg.tar.zst";
    sha256 = "0g351clmh8jkpmp25gcc6527b078ldksh1lcp57bk1l0p2y20h6w";
    name = "mingw-w64-clang-x86_64-llvm-openmp-22.1.7-1-any.pkg.tar.zst";
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
    url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-81.0.0-1-any.pkg.tar.zst";
    sha256 = "1f7bwgnnqn6p9i2kj7ldl2wz1d4wx163iskrlsfpjfr84anh38lg";
    name = "mingw-w64-clang-x86_64-python-setuptools-81.0.0-1-any.pkg.tar.zst";
  })
]
