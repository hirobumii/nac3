{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libwinpthread-git-11.0.0.r147.gddc5b0f6e-1-any.pkg.tar.zst";
  sha256 = "0w4lizwnl41pff298mkm7ylndcdjc8655fk2hlqyys87vjf55rsv";
  name = "mingw-w64-x86_64-libwinpthread-git-11.0.0.r147.gddc5b0f6e-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libs-13.2.0-2-any.pkg.tar.zst";
  sha256 = "0bxdyy0w0ld437skyl6wmp9d1j2jj71s8sw7wcysgv40y72b9jxc";
  name = "mingw-w64-x86_64-gcc-libs-13.2.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
  sha256 = "19jr14l5anl1qr1lvcmdpnpgyxghf2mds2j7iiq4j99kfg7ig2s0";
  name = "mingw-w64-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-binutils-2.41-2-any.pkg.tar.zst";
  sha256 = "1r2pf8sdhhs8mlyzagfl209d6xa92yqf0nzqvbd0ggapijni02ga";
  name = "mingw-w64-x86_64-binutils-2.41-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-headers-git-11.0.0.r147.gddc5b0f6e-1-any.pkg.tar.zst";
  sha256 = "0qbikziflz8vh4gh4d1v1vzxw9v97nzbvrhl4yj6r73apqyga2gy";
  name = "mingw-w64-x86_64-headers-git-11.0.0.r147.gddc5b0f6e-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-crt-git-11.0.0.r147.gddc5b0f6e-1-any.pkg.tar.zst";
  sha256 = "1f0d5n0cykd9j1dazr34sglrfdp47lbdm8ky3rxyp4lr9jrdkgvx";
  name = "mingw-w64-x86_64-crt-git-11.0.0.r147.gddc5b0f6e-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gmp-6.3.0-1-any.pkg.tar.zst";
  sha256 = "0v65a02mwxj6y8hcwm25pcr3lyv1jnw1n3b7sl1fbfv50sx6rwyc";
  name = "mingw-w64-x86_64-gmp-6.3.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-isl-0.26-1-any.pkg.tar.zst";
  sha256 = "0hfycibi23xkah9sw60n4ka786l8vmlc67d2waw9mwqqljdmx7pr";
  name = "mingw-w64-x86_64-isl-0.26-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libiconv-1.17-3-any.pkg.tar.zst";
  sha256 = "061dlpg69ph2205xabshya827m6dqchxxn3jvhnnicja6bsb8ivh";
  name = "mingw-w64-x86_64-libiconv-1.17-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpfr-4.2.1-1-any.pkg.tar.zst";
  sha256 = "08jiasd2v831hip356vvw9yarzv15jsdqbill26rk6xshcnzapdx";
  name = "mingw-w64-x86_64-mpfr-4.2.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpc-1.3.1-1-any.pkg.tar.zst";
  sha256 = "1r7h4xyc56d9n4z6ay315gsb82zmyvrkd7xki9y03y72ym194jlk";
  name = "mingw-w64-x86_64-mpc-1.3.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-windows-default-manifest-6.4-4-any.pkg.tar.zst";
  sha256 = "1ylipf8k9j7bgmwndkib2l29mds394i7jcij7a6ciag4kynlhsvi";
  name = "mingw-w64-x86_64-windows-default-manifest-6.4-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-winpthreads-git-11.0.0.r147.gddc5b0f6e-1-any.pkg.tar.zst";
  sha256 = "00bdidq17csfmq9bx2nckmy8g2dj75a32bl980nk0k9cpgz6in0s";
  name = "mingw-w64-x86_64-winpthreads-git-11.0.0.r147.gddc5b0f6e-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zlib-1.3-1-any.pkg.tar.zst";
  sha256 = "167g32vk257sbfmz85azgjs01cnfkjip0gks6y3vgl97i9d6qji5";
  name = "mingw-w64-x86_64-zlib-1.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-13.2.0-2-any.pkg.tar.zst";
  sha256 = "09v6iz3iyp8w3ynw7wrs2qh54vr2kjpwfbv5rya8dskdx4sp2bvd";
  name = "mingw-w64-x86_64-gcc-13.2.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-c-ares-1.19.1-1-any.pkg.tar.zst";
  sha256 = "0mnkybl3ymxljvg4z5lnww14k29axyxg6ww30mxz6p5i2kqb0vik";
  name = "mingw-w64-x86_64-c-ares-1.19.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-brotli-1.1.0-1-any.pkg.tar.zst";
  sha256 = "1ix63yg59k6wq32xgs64i3i2hqsi9f5qj5qw5apsfr1sgy9zlppm";
  name = "mingw-w64-x86_64-brotli-1.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-expat-2.5.0-1-any.pkg.tar.zst";
  sha256 = "09hrzvdfkr2zaq239z87m1j3zyq0pvjhsyikg65wrbljrir6wc6r";
  name = "mingw-w64-x86_64-expat-2.5.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gettext-0.21.1-2-any.pkg.tar.zst";
  sha256 = "0gj9qgxph9qw1x3y9ijacxi4ia90vzgkmg5jvl99pdq55h3xxl9x";
  name = "mingw-w64-x86_64-gettext-0.21.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libunistring-1.1-1-any.pkg.tar.zst";
  sha256 = "1zpmarlb2j0q2hcv30xl6c0mm3pwdjp7fh9mqpb6y0yygj1ivcza";
  name = "mingw-w64-x86_64-libunistring-1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libidn2-2.3.4-1-any.pkg.tar.zst";
  sha256 = "0z926vsxz61m5zxdarah3zc4n253ksykxvb72qg86kcxcl3z0ppc";
  name = "mingw-w64-x86_64-libidn2-2.3.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libpsl-0.21.2-4-any.pkg.tar.zst";
  sha256 = "0scpar3qp91y920c065y7jcvzfpmxx5vva9ybgxkk4df8a8mrbs9";
  name = "mingw-w64-x86_64-libpsl-0.21.2-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libtasn1-4.19.0-1-any.pkg.tar.zst";
  sha256 = "09bgm2y25jyjm0pwn2imnr30nxzdd7j71ifmxkpabaqkpsfa5av5";
  name = "mingw-w64-x86_64-libtasn1-4.19.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libffi-3.4.4-1-any.pkg.tar.zst";
  sha256 = "1na3giynh9f3i0xg2mr0dm4bm6zhv8h908rrrv4kcxfawr8nyjdy";
  name = "mingw-w64-x86_64-libffi-3.4.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-p11-kit-0.25.0-2-any.pkg.tar.zst";
  sha256 = "0j6w4hijyclv2av8wxldqdznyyycsmx00kshbs8kpk2wd5gz1scs";
  name = "mingw-w64-x86_64-p11-kit-0.25.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ca-certificates-20230311-1-any.pkg.tar.zst";
  sha256 = "1rcxxlpmvian8c2d9bmnx5hldzvzz4wybz5wp9pjryps1rmzylyx";
  name = "mingw-w64-x86_64-ca-certificates-20230311-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openssl-3.1.2-1-any.pkg.tar.zst";
  sha256 = "0qywbh5x3lmqi7lyn3did5ahlkrd628rg2s2vph062x7sks81h2h";
  name = "mingw-w64-x86_64-openssl-3.1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
  sha256 = "0h4hfsig3n7grp7hn7vn16af6x122hc220llpmd8aii3d3jwc8d1";
  name = "mingw-w64-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-nghttp2-1.56.0-1-any.pkg.tar.zst";
  sha256 = "1jxn8g0w5qlkfb30bllwsm8vjfnk5952x8z8rq3wbawdncwsgvmk";
  name = "mingw-w64-x86_64-nghttp2-1.56.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-curl-8.2.1-1-any.pkg.tar.zst";
  sha256 = "1yw9r6fzammvzkqmp0rd3m3x65hv87730l1i95hpm7d0xnrivfwr";
  name = "mingw-w64-x86_64-curl-8.2.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-xz-5.4.4-1-any.pkg.tar.zst";
  sha256 = "0drcmy0x3dydl19slxv64aw8f29a1kxyzx7zj25nnr47qg1w5ycp";
  name = "mingw-w64-x86_64-xz-5.4.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libxml2-2.11.5-1-any.pkg.tar.zst";
  sha256 = "0z1nynr2ip1js3p8rlj5da1sfrm1071s6cn22qwpzk3j6zfjzanv";
  name = "mingw-w64-x86_64-libxml2-2.11.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rust-1.72.0-1-any.pkg.tar.zst";
  sha256 = "1vhr1paymsc0spbv0yg2hlzayq5swcci5i5c6zj1gg4g92bi6h6q";
  name = "mingw-w64-x86_64-rust-1.72.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-pkgconf-1~2.0.3-1-any.pkg.tar.zst";
  sha256 = "0ia3r14yghq21yzwi4vnpq6h9qy1bwdjv5wqm97gx3n6jxy63zwa";
  name = "mingw-w64-x86_64-pkgconf-12.0.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-jsoncpp-1.9.5-2-any.pkg.tar.zst";
  sha256 = "0wjf5cycjxwbaxvk4xmzhj4hnpl1mq6ddqj5lcbdcrvsc13nj8ll";
  name = "mingw-w64-x86_64-jsoncpp-1.9.5-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-bzip2-1.0.8-2-any.pkg.tar.zst";
  sha256 = "1kqg3aw439cdyhnf02rlfr1pw1n8v9xxvq2alhn7aw6nd8qhw7z5";
  name = "mingw-w64-x86_64-bzip2-1.0.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libb2-0.98.1-2-any.pkg.tar.zst";
  sha256 = "1nj669rn1i6fxrwmsqmr9n49p34wxvhn0xlsn9spr6aq1hz73b41";
  name = "mingw-w64-x86_64-libb2-0.98.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-lz4-1.9.4-1-any.pkg.tar.zst";
  sha256 = "1mwyd94pwp1j3pgaa7j2i37d1xid1ynr0a42fl2pxgfmcj6hmqfi";
  name = "mingw-w64-x86_64-lz4-1.9.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libtre-git-r128.6fb7206-2-any.pkg.tar.xz";
  sha256 = "0dp3ca83j8jlx32gml2qvqpwp5b42q8r98gf6hyiki45d910wb7x";
  name = "mingw-w64-x86_64-libtre-git-r128.6fb7206-2-any.pkg.tar.xz";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libsystre-1.0.1-4-any.pkg.tar.xz";
  sha256 = "037gkzaaj8kp5nspcbc8ll64s9b3mj8d6m663lk1za94bq2axff1";
  name = "mingw-w64-x86_64-libsystre-1.0.1-4-any.pkg.tar.xz";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libarchive-3.7.2-1-any.pkg.tar.zst";
  sha256 = "0p302zs5jgbbrv368yyjvbjmj4bmc1qxb0dp7s4wwafwnjqfm3vw";
  name = "mingw-w64-x86_64-libarchive-3.7.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libuv-1.46.0-1-any.pkg.tar.zst";
  sha256 = "1pasn07awq2mrqzsf1162aa5xlq81745mxkzir0z7cx6smrfqiwb";
  name = "mingw-w64-x86_64-libuv-1.46.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ninja-1.11.1-3-any.pkg.tar.zst";
  sha256 = "0494d54qxax9d2gz11vhm7342311k4s6mf6zy5yq2ka07qfzckcg";
  name = "mingw-w64-x86_64-ninja-1.11.1-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rhash-1.4.3-1-any.pkg.tar.zst";
  sha256 = "1nd0iqlx1vmn079i24i07r4kqfr3yr0apnzsgcx8qd5cyvwnl7w6";
  name = "mingw-w64-x86_64-rhash-1.4.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-cmake-3.27.4-2-any.pkg.tar.zst";
  sha256 = "1xh5l57mbbnj63cdzjliv3wnrpciwvimrfav9g7cl99r7sna40y9";
  name = "mingw-w64-x86_64-cmake-3.27.4-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
  sha256 = "0cpyacmciyzbsar1aka5y592g2gpa4i6a58j3bjdmfjdnpm0j08a";
  name = "mingw-w64-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ncurses-6.4.20230708-1-any.pkg.tar.zst";
  sha256 = "1bpjn6rv85q3rdcdgs7fml140aar93hv649hhqx47za26mjnsdiv";
  name = "mingw-w64-x86_64-ncurses-6.4.20230708-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-termcap-1.3.1-6-any.pkg.tar.zst";
  sha256 = "1wgbzj53vmv1vm3igjan635j5ims4x19s2y6mgvvc46zgndc2bvq";
  name = "mingw-w64-x86_64-termcap-1.3.1-6-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-readline-8.2.001-6-any.pkg.tar.zst";
  sha256 = "0a6s6kq2hmz96cg7hxzcgldh16sk7dvpzfdfqchq3c07rwzhqhiq";
  name = "mingw-w64-x86_64-readline-8.2.001-6-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tcl-8.6.12-2-any.pkg.tar.zst";
  sha256 = "17lyrkyh76lh0ghk19d91kwicms6lxshhhb6n3zh748awfvihknm";
  name = "mingw-w64-x86_64-tcl-8.6.12-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-sqlite3-3.43.0-1-any.pkg.tar.zst";
  sha256 = "13yl691gsha7kmwlb1rjgljvc6kb4mqkywbj9ljdl5jkc21vgqkb";
  name = "mingw-w64-x86_64-sqlite3-3.43.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tk-8.6.12-2-any.pkg.tar.zst";
  sha256 = "0j6nvwc0a1cc2k4akq3095r1rfhprslf8jpr07ypcjb91q5s3yfi";
  name = "mingw-w64-x86_64-tk-8.6.12-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tzdata-2023c-1-any.pkg.tar.zst";
  sha256 = "0ifavpqi1ykn9962ic4sh5l18y2mvz9pj6742fgw85s9wixbj7fl";
  name = "mingw-w64-x86_64-tzdata-2023c-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-3.11.5-2-any.pkg.tar.zst";
  sha256 = "0i926qzh27pd47jp12nyf0ypm15kc2km90rymj5c1z3zbzwfyn0k";
  name = "mingw-w64-x86_64-python-3.11.5-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libgfortran-13.2.0-2-any.pkg.tar.zst";
  sha256 = "150k73v8g71wrp855f747mal009cf34lbpv8xzbibj50m3g6yxvv";
  name = "mingw-w64-x86_64-gcc-libgfortran-13.2.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openblas-0.3.24-1-any.pkg.tar.zst";
  sha256 = "0j2aka8bml2j2aszxsjy5gp8xqvdxw3s292pd7iarzn1kliwv84j";
  name = "mingw-w64-x86_64-openblas-0.3.24-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-numpy-1.25.2-1-any.pkg.tar.zst";
  sha256 = "0vjb2qps0kzzcjhjrh11h661i9q1hmzi08jvirqyslayp3jmbkd9";
  name = "mingw-w64-x86_64-python-numpy-1.25.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-setuptools-68.1.2-1-any.pkg.tar.zst";
  sha256 = "1d44yn8qbkma1k20s6lh7qhdcgz48wjsk18ka02w9i7x450qr0vn";
  name = "mingw-w64-x86_64-python-setuptools-68.1.2-1-any.pkg.tar.zst";
})
]
