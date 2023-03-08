{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libwinpthread-git-10.0.0.r234.g283e5b23a-1-any.pkg.tar.zst";
  sha256 = "1s601hn3i668p8nda14bg3sdc3j6nrqca4ns84ybp5p12xsy225w";
  name = "mingw-w64-x86_64-libwinpthread-git-10.0.0.r234.g283e5b23a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libs-12.2.0-10-any.pkg.tar.zst";
  sha256 = "1hql9jmmcpdr97p2ynj45hb70az4l41hcirjk0j1xjg31m2jgfr9";
  name = "mingw-w64-x86_64-gcc-libs-12.2.0-10-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zstd-1.5.4-1-any.pkg.tar.zst";
  sha256 = "0ps42vy3wjmspz4glb492x0x7g3fcgv9whx53ggl3idjwhbk46lx";
  name = "mingw-w64-x86_64-zstd-1.5.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-binutils-2.40-2-any.pkg.tar.zst";
  sha256 = "1xa3ni7qg9wzlr903lsqgqisdyvnl28kb3wz2kva21l9i7wwbs7c";
  name = "mingw-w64-x86_64-binutils-2.40-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-headers-git-10.0.0.r234.g283e5b23a-1-any.pkg.tar.zst";
  sha256 = "09i1r4nyrficrv39xh60y8jayfl05xvgj9sm27cp9f97gxyz8s33";
  name = "mingw-w64-x86_64-headers-git-10.0.0.r234.g283e5b23a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-crt-git-10.0.0.r234.g283e5b23a-1-any.pkg.tar.zst";
  sha256 = "1vcg8j5p0w1jqf9hncadsas6k6d9z66ci3nk4y3qhnl2sj1zz924";
  name = "mingw-w64-x86_64-crt-git-10.0.0.r234.g283e5b23a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gmp-6.2.1-5-any.pkg.tar.zst";
  sha256 = "1v19jx0hrsib7ak5jzbhss7p5zjg9y4kj06bcs8sakqvmyby8mlq";
  name = "mingw-w64-x86_64-gmp-6.2.1-5-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-isl-0.25-1-any.pkg.tar.zst";
  sha256 = "0hky9gmd6iz1s3irmp9fk2j10cpqrrw8l810riwr58ynj3i10j2k";
  name = "mingw-w64-x86_64-isl-0.25-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libiconv-1.17-3-any.pkg.tar.zst";
  sha256 = "061dlpg69ph2205xabshya827m6dqchxxn3jvhnnicja6bsb8ivh";
  name = "mingw-w64-x86_64-libiconv-1.17-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpfr-4.2.0-1-any.pkg.tar.zst";
  sha256 = "1iqpk6i5isf77rmvscmdv1ggrnhbvbfc3g4cyc6xgbp99s616724";
  name = "mingw-w64-x86_64-mpfr-4.2.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-winpthreads-git-10.0.0.r234.g283e5b23a-1-any.pkg.tar.zst";
  sha256 = "018bh811zrj7fd47p4fj4fawja7n0s129ghv91rwpxif0kj6b5bf";
  name = "mingw-w64-x86_64-winpthreads-git-10.0.0.r234.g283e5b23a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zlib-1.2.13-3-any.pkg.tar.zst";
  sha256 = "19r9hf111zb41i7r45ixsx26l4sk8g8apryv1xgj03hq54barikz";
  name = "mingw-w64-x86_64-zlib-1.2.13-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-12.2.0-10-any.pkg.tar.zst";
  sha256 = "182560g1bl52260v8dbggybl4mir3isyad22zvkb6sndid3iaakw";
  name = "mingw-w64-x86_64-gcc-12.2.0-10-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-c-ares-1.19.0-1-any.pkg.tar.zst";
  sha256 = "0h9gpqr08rpil1a4cjd2ajk2is2fzgbhwg2n7va9jl2zfxksd6my";
  name = "mingw-w64-x86_64-c-ares-1.19.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-brotli-1.0.9-5-any.pkg.tar.zst";
  sha256 = "044n36p4s2n73fxvac55cqqw6di19v4m92v2h0qnphazj6wcg1d0";
  name = "mingw-w64-x86_64-brotli-1.0.9-5-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-expat-2.5.0-1-any.pkg.tar.zst";
  sha256 = "09hrzvdfkr2zaq239z87m1j3zyq0pvjhsyikg65wrbljrir6wc6r";
  name = "mingw-w64-x86_64-expat-2.5.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gettext-0.21.1-1-any.pkg.tar.zst";
  sha256 = "17h4qnv75jns7fq54hqp375v45snmrrn451izyp2nmmr0fw2p0bc";
  name = "mingw-w64-x86_64-gettext-0.21.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-p11-kit-0.24.1-3-any.pkg.tar.zst";
  sha256 = "18ghwd6sy15hjp0si0ia85yvpv0fnawjdn8lxg3yyr93c6hdfssz";
  name = "mingw-w64-x86_64-p11-kit-0.24.1-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ca-certificates-20211016-4-any.pkg.tar.zst";
  sha256 = "0nj31jjl2qs9z209na154a6zc38zdrv9gzywdcaayd7li0rh9l7a";
  name = "mingw-w64-x86_64-ca-certificates-20211016-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openssl-3.0.8-1-any.pkg.tar.zst";
  sha256 = "11v63md015nsqci5wnvx3cfxlminw4zhipd337xzp439bsihy11n";
  name = "mingw-w64-x86_64-openssl-3.0.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libssh2-1.10.0-2-any.pkg.tar.zst";
  sha256 = "0q1l2258063b8byyh1il864nz76m1q8q820k1qds0c3n1s9zdm6f";
  name = "mingw-w64-x86_64-libssh2-1.10.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-nghttp2-1.52.0-1-any.pkg.tar.zst";
  sha256 = "0w0z9a8ahhij2sdqyxkynahi71w69kw6pw692fwc7vcxjnd1bj2v";
  name = "mingw-w64-x86_64-nghttp2-1.52.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-curl-7.88.1-1-any.pkg.tar.zst";
  sha256 = "1y7xpmyf1dbfkmd7l7wwh7l7f576ww96hw56yi0w736zlz83nddz";
  name = "mingw-w64-x86_64-curl-7.88.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-xz-5.4.1-1-any.pkg.tar.zst";
  sha256 = "0n1dfc5cy9ya13mp8hx5pm0qskb1q6dkl6mhmvz4kaynw7c94p6y";
  name = "mingw-w64-x86_64-xz-5.4.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libxml2-2.10.3-1-any.pkg.tar.zst";
  sha256 = "087835d8lg19drq9wcn9fpbdvai0pcsh6layhvd9zh67bgpgyaq9";
  name = "mingw-w64-x86_64-libxml2-2.10.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rust-1.67.1-1-any.pkg.tar.zst";
  sha256 = "1kr5gyajy1r8hqnv90fdlqysm0i5kl3p0d62pmpj7xf16mkvhzsx";
  name = "mingw-w64-x86_64-rust-1.67.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-pkgconf-1~1.8.0-2-any.pkg.tar.zst";
  sha256 = "1w9nx52h37awlj8ac068y844jw4lb55vfjphk9hg5l6yqa036bvn";
  name = "mingw-w64-x86_64-pkgconf-11.8.0-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libarchive-3.6.2-2-any.pkg.tar.zst";
  sha256 = "1j8rm8zk0b7wg20cbw3f0nll7m42clk5m1gl163m5a83r4s8wmnn";
  name = "mingw-w64-x86_64-libarchive-3.6.2-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libuv-1.44.2-2-any.pkg.tar.zst";
  sha256 = "143qq7373x4zpha1nksa7ah7hxz0qirgdj1s09pb3hcap1ijbjp2";
  name = "mingw-w64-x86_64-libuv-1.44.2-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-cmake-3.25.2-1-any.pkg.tar.zst";
  sha256 = "0qz7mcl2p0lghbi7hmlb1am0vbac9xiyr22hr0w37fpiz3i5l072";
  name = "mingw-w64-x86_64-cmake-3.25.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
  sha256 = "0cpyacmciyzbsar1aka5y592g2gpa4i6a58j3bjdmfjdnpm0j08a";
  name = "mingw-w64-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ncurses-6.4.20230211-1-any.pkg.tar.zst";
  sha256 = "0h62y3c45bkff6z3aa8ailz2l16x3s9g3pbyifqx6kwwzv80crgp";
  name = "mingw-w64-x86_64-ncurses-6.4.20230211-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tcl-8.6.12-1-any.pkg.tar.zst";
  sha256 = "0z66xic67k3j56jvvrwn8sym5yxylyza7ig686z5937nsd29kdw1";
  name = "mingw-w64-x86_64-tcl-8.6.12-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-sqlite3-3.41.0-1-any.pkg.tar.zst";
  sha256 = "13168pp7w1wzkabd0vl7khxaq367rkizjf07cfixgn025c22zs0y";
  name = "mingw-w64-x86_64-sqlite3-3.41.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tk-8.6.12-1-any.pkg.tar.zst";
  sha256 = "1pnznf4a195ij3b1g921k0llkn62wf0piijldj2c7qlbcq73v66c";
  name = "mingw-w64-x86_64-tk-8.6.12-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tzdata-2022g-1-any.pkg.tar.zst";
  sha256 = "0lww8lgw8q4wp7r1zqilcqs26ircsv5gl97rcf3b2zfgbfdjyc76";
  name = "mingw-w64-x86_64-tzdata-2022g-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-3.10.10-1-any.pkg.tar.zst";
  sha256 = "14whllm3cs0lsx9l158jzdvh476ri74l7yxdhr4a4js1s65hkyx1";
  name = "mingw-w64-x86_64-python-3.10.10-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libgfortran-12.2.0-10-any.pkg.tar.zst";
  sha256 = "1xilwrasyj3xbrv4wjvc53bv45k7szpzgnnakdnl1jg81960byx5";
  name = "mingw-w64-x86_64-gcc-libgfortran-12.2.0-10-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openblas-0.3.21-7-any.pkg.tar.zst";
  sha256 = "09pr50afm2xbbalvfxv5vvaf48sckacb6qwr88dghyjkiqk0wds8";
  name = "mingw-w64-x86_64-openblas-0.3.21-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-numpy-1.24.2-1-any.pkg.tar.zst";
  sha256 = "1dzfvhvzfhcmi9jx163qiaqaalj92xn09cf12dwyndjh2vrabipc";
  name = "mingw-w64-x86_64-python-numpy-1.24.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-setuptools-67.5.0-1-any.pkg.tar.zst";
  sha256 = "11n3ijx4gmam42qd1nwmcgf6n0n1xysqblibkp4mv428j9rbj7rp";
  name = "mingw-w64-x86_64-python-setuptools-67.5.0-1-any.pkg.tar.zst";
})
]
