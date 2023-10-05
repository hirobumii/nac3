{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libwinpthread-git-11.0.0.r198.g93ca95b32-1-any.pkg.tar.zst";
  sha256 = "1zv1s7jamj6m4b7l05s185cslyiclp1r5vhxv7lj16gz21n800vg";
  name = "mingw-w64-x86_64-libwinpthread-git-11.0.0.r198.g93ca95b32-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-headers-git-11.0.0.r198.g93ca95b32-1-any.pkg.tar.zst";
  sha256 = "19sgcf61vrs5cpzxskb6g279srsc49mhq4840snf45lrsxfrk6ja";
  name = "mingw-w64-x86_64-headers-git-11.0.0.r198.g93ca95b32-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-crt-git-11.0.0.r198.g93ca95b32-1-any.pkg.tar.zst";
  sha256 = "04717li07m2f3q608ir83vqayixw38mkr9sjk2xjhaam5hhl96lq";
  name = "mingw-w64-x86_64-crt-git-11.0.0.r198.g93ca95b32-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gmp-6.3.0-2-any.pkg.tar.zst";
  sha256 = "1k0ma22hyn5m2m8kflpmscwm2p1v53pzd93fnind9bf4fhwl6949";
  name = "mingw-w64-x86_64-gmp-6.3.0-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpfr-4.2.1-2-any.pkg.tar.zst";
  sha256 = "1j96kipr7mzawngjhi9m0rh2lhylmggg1mkgkipw9ssrsxxf7g97";
  name = "mingw-w64-x86_64-mpfr-4.2.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpc-1.3.1-2-any.pkg.tar.zst";
  sha256 = "04md7pzz6rwvlsxzgxn8zc6l5lmqn1w2dg9f5xdf13qbl9zfm615";
  name = "mingw-w64-x86_64-mpc-1.3.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-windows-default-manifest-6.4-4-any.pkg.tar.zst";
  sha256 = "1ylipf8k9j7bgmwndkib2l29mds394i7jcij7a6ciag4kynlhsvi";
  name = "mingw-w64-x86_64-windows-default-manifest-6.4-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-winpthreads-git-11.0.0.r198.g93ca95b32-1-any.pkg.tar.zst";
  sha256 = "171blcxd721pp0881blbid8vdbc18fx2lwyzk2prsnjhlf9yampj";
  name = "mingw-w64-x86_64-winpthreads-git-11.0.0.r198.g93ca95b32-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openssl-3.1.3-1-any.pkg.tar.zst";
  sha256 = "1vi1zxb5lvgxpmk80giqahqrjh2lv06r2ah4hf55jzjsi1mmcard";
  name = "mingw-w64-x86_64-openssl-3.1.3-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-curl-8.3.0-1-any.pkg.tar.zst";
  sha256 = "03gkb36fp079gd2hi980qk1k2baly17ca457ah1gjcmc6p56jx6d";
  name = "mingw-w64-x86_64-curl-8.3.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rust-1.72.1-1-any.pkg.tar.zst";
  sha256 = "0dq01ah76ra5y64kp1xwjhy5rsv7fxh1hnfjzr6l3x26d1hjwgsi";
  name = "mingw-w64-x86_64-rust-1.72.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-pkgconf-1~2.0.3-2-any.pkg.tar.zst";
  sha256 = "0wkgwk57d6kyljjs0zvlrjp7k87s9b061cvzy0b8iy7r8wql31py";
  name = "mingw-w64-x86_64-pkgconf-12.0.3-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-jsoncpp-1.9.5-2-any.pkg.tar.zst";
  sha256 = "0wjf5cycjxwbaxvk4xmzhj4hnpl1mq6ddqj5lcbdcrvsc13nj8ll";
  name = "mingw-w64-x86_64-jsoncpp-1.9.5-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-bzip2-1.0.8-3-any.pkg.tar.zst";
  sha256 = "1dki26kz4pmr9q3gp3dirrvrwkcv38b9sjrb9slrq4yw31ycjgk5";
  name = "mingw-w64-x86_64-bzip2-1.0.8-3-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-cmake-3.27.6-1-any.pkg.tar.zst";
  sha256 = "0vhnij28yk7i068b65m1gsmswz0r8nl3vzvx3c9n3cncv6h6m5hs";
  name = "mingw-w64-x86_64-cmake-3.27.6-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
  sha256 = "1s51i2fwy1mrzmxsgr1vv87wlmb3bk88yipqalfldvy3xdgjgjh4";
  name = "mingw-w64-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-sqlite3-3.43.1-1-any.pkg.tar.zst";
  sha256 = "05csh3sa6g5nnfmry50w6p2j4v7fqylmlgnny5cimm350ppmmkka";
  name = "mingw-w64-x86_64-sqlite3-3.43.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-3.11.6-1-any.pkg.tar.zst";
  sha256 = "0s4z65i3p0arwjry877q9q154h58b3wd35svifpj2c1j7frr41vi";
  name = "mingw-w64-x86_64-python-3.11.6-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-setuptools-68.2.2-1-any.pkg.tar.zst";
  sha256 = "0hci3vran7cw4hnr17b04xcpwl322ffvxf9sgv3793rpx7fx5h7a";
  name = "mingw-w64-x86_64-python-setuptools-68.2.2-1-any.pkg.tar.zst";
})
]
