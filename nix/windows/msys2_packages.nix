{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libiconv-1.17-1-any.pkg.tar.zst";
  sha256 = "1pb1x5wrlmmpjdpzsc7rs5xk6ydlsd5mval0fwrqq54jf6dxdzpz";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zlib-1.2.12-1-any.pkg.tar.zst";
  sha256 = "1b461ic5s3hjk3y70ldik82ny08rdywn1zfqa8d2jyyvnh4dya77";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-binutils-2.38-4-any.pkg.tar.zst";
  sha256 = "18cgs1cvhr8hrq46g2av9as589wxn76rrshhzvx8max8iqzwprm3";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-headers-git-10.0.0.r59.gaacb650be-1-any.pkg.tar.zst";
  sha256 = "0gq38zb880ar0xj62ddcggw8cqg7h6g1yw0x422i8cgak6x8qasp";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-crt-git-10.0.0.r59.gaacb650be-1-any.pkg.tar.zst";
  sha256 = "1safighnniwmjrklrig41m1kj1b40lrzaiv48xzf26ljb45fy6lq";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gmp-6.2.1-3-any.pkg.tar.zst";
  sha256 = "170640c8j81gl67kp85kr8kmg5axsl1vqwn9g7cx6vcr638qax9c";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-isl-0.25-1-any.pkg.tar.zst";
  sha256 = "0hky9gmd6iz1s3irmp9fk2j10cpqrrw8l810riwr58ynj3i10j2k";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpfr-4.1.0.p13-1-any.pkg.tar.zst";
  sha256 = "17klcf17mddd7hsrak920zglqh00drqjdh6dxh3v3c4y62xj1qr6";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpc-1.2.1-1-any.pkg.tar.zst";
  sha256 = "0761i6aga4982v6mw1hgqrrqznki0c8v93xkpf5fqmsjysfncscc";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libwinpthread-git-10.0.0.r59.gaacb650be-1-any.pkg.tar.zst";
  sha256 = "0a9niq05s7ny0y1x625xy9p3dzakw5l4w8djajv09lkxqx36yp40";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libs-12.1.0-3-any.pkg.tar.zst";
  sha256 = "0gxifzjl9v72z5fbr89j47j2b7l7ba9cf4xf49wb3khppqb2q9by";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-windows-default-manifest-6.4-4-any.pkg.tar.zst";
  sha256 = "1ylipf8k9j7bgmwndkib2l29mds394i7jcij7a6ciag4kynlhsvi";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-winpthreads-git-10.0.0.r59.gaacb650be-1-any.pkg.tar.zst";
  sha256 = "1mhy806hdx27w3fzpb4zv9ia0c2r6n53ljcpkpcanwbqc3hhmj9f";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zstd-1.5.2-2-any.pkg.tar.zst";
  sha256 = "1f14wbc1yvjgv3rbwhv75391l55gcm0as6ipba20vw8phz4ax8ds";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-12.1.0-3-any.pkg.tar.zst";
  sha256 = "0j2p4516r7r9igcnfjcxyzzgppy60hx76gp78lqk0331aj1c5d1d";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-c-ares-1.18.1-1-any.pkg.tar.zst";
  sha256 = "13j7zx9773k0gx3wbqq38jkcndyjpnm7dfb85i8q2dda27g4iq2m";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-brotli-1.0.9-5-any.pkg.tar.zst";
  sha256 = "044n36p4s2n73fxvac55cqqw6di19v4m92v2h0qnphazj6wcg1d0";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-expat-2.4.8-1-any.pkg.tar.zst";
  sha256 = "1qkw4k61ddaflns5ms0xh0czbx99wxhs0dfbk8sv8by2rkshl51k";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gettext-0.21-3-any.pkg.tar.zst";
  sha256 = "1gy7fmn6jc13ipnyyq44gyhv8rvz5cy7gz1dm3wrna80hjnzli5v";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libunistring-1.0-1-any.pkg.tar.zst";
  sha256 = "1qks1gm8jscnn93sr7n1azkzcq4a8fybsikpqcf920m9b66cym4k";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libidn2-2.3.3-1-any.pkg.tar.zst";
  sha256 = "1m3qgnhgf0g389kglrai26x4k64gs2cy9b3mjwlkw5xcs2r3smww";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libpsl-0.21.1-4-any.pkg.tar.zst";
  sha256 = "083nng8zis1v2lshnqymxnalprr8g6gdwf84il5ys1ga90pi6bbn";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libtasn1-4.18.0-1-any.pkg.tar.zst";
  sha256 = "0lr1c33d2mkm51kq027bxcj2735vk3nndmn8g02d2v73h6akl48k";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libffi-3.3-4-any.pkg.tar.zst";
  sha256 = "0iswfiql785ngavdz3qdxahj6wn531j5cwij945gbr9q6wbav0bi";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-p11-kit-0.24.1-2-any.pkg.tar.zst";
  sha256 = "1rx1sjhda4g42qsvmw9nvpdk14ag67sxgfiydivg55hxqjxsvk9p";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ca-certificates-20211016-3-any.pkg.tar.zst";
  sha256 = "02x6dnbbyjm6mcl6ii61bc5rkwg3qsbaqd2lyzsp5732hxjcmmq4";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openssl-1.1.1.q-1-any.pkg.tar.zst";
  sha256 = "0rfb3z9jd0y6xjhv4qx1qqyyqgnzzlchbm07icpb4slgwjbm7cjg";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libssh2-1.10.0-1-any.pkg.tar.zst";
  sha256 = "1f27an41hxrfs9jifq0708c484ps3zmb582gmsy7xn5idg3wk03d";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-jansson-2.14-2-any.pkg.tar.zst";
  sha256 = "0hwvcyp7mcvljii87mv0d467whr5j8i8rjkkam7r784qrp9i49ds";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-jemalloc-5.2.1-2-any.pkg.tar.zst";
  sha256 = "04lf1b1sdbb8ncbimbb9q0lv7qlc3s814p2001zsxa2dhdc4xdri";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-xz-5.2.5-3-any.pkg.tar.zst";
  sha256 = "099j96iv49b2xddfaq7a69l0j818hw7cxyas6g7cm7iw3crsykfr";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libxml2-2.9.14-4-any.pkg.tar.zst";
  sha256 = "1d6v37k0hiznlv0qnr25cpjgwa7rphahiwcrc7jf44qwdmbdasrv";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-nghttp2-1.48.0-1-any.pkg.tar.zst";
  sha256 = "023lnhncm697sdbgnrzvc56c9lzcn29dsrl1m58hsxxjb7rrcrlf";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-curl-7.84.0-2-any.pkg.tar.zst";
  sha256 = "0j6b3arlcsyk5fn2nr7x92j2pqkn26zyrg1zy3pc0qcd3q8hlbr0";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rust-1.62.1-1-any.pkg.tar.zst";
  sha256 = "1bxnjgf1vx1qyf0nzmmc6s096jbw7354fkb1khhmldi15yb2f8h8";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-pkgconf-1.8.0-2-any.pkg.tar.zst";
  sha256 = "18gzbyc949rvaisdxrf4lyx349xigzpp4dk5a9jj9ghn8zfa1wlg";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-jsoncpp-1.9.4-2-any.pkg.tar.zst";
  sha256 = "0xqckav97gsaazdfn4395jz0ma0i3snvs1g4ghb7s5jsxbwrhr82";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-bzip2-1.0.8-2-any.pkg.tar.zst";
  sha256 = "1kqg3aw439cdyhnf02rlfr1pw1n8v9xxvq2alhn7aw6nd8qhw7z5";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libb2-0.98.1-2-any.pkg.tar.zst";
  sha256 = "1nj669rn1i6fxrwmsqmr9n49p34wxvhn0xlsn9spr6aq1hz73b41";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-lz4-1.9.3-1-any.pkg.tar.zst";
  sha256 = "0fxvabi93cxfybbn49hlr3wgzs4p7fw5shfa055222apkxnncm92";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libtre-git-r128.6fb7206-2-any.pkg.tar.xz";
  sha256 = "0dp3ca83j8jlx32gml2qvqpwp5b42q8r98gf6hyiki45d910wb7x";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libsystre-1.0.1-4-any.pkg.tar.xz";
  sha256 = "037gkzaaj8kp5nspcbc8ll64s9b3mj8d6m663lk1za94bq2axff1";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libarchive-3.6.1-2-any.pkg.tar.zst";
  sha256 = "1wgv99pxk2pv4kr5cs111k7813bvlykphirksz8pr62kv8a1n47s";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libuv-1.42.0-3-any.pkg.tar.zst";
  sha256 = "0mg4j5lqmxlhgrs9bnkb1bhj3mfpvjvvkzpjyy87y2m2k11ffbja";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rhash-1.4.2-1-any.pkg.tar.zst";
  sha256 = "0yhv8pra83cs0mk0n40w0k12z32slxs88h1p9z2ixvyigf8w86ml";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ninja-1.11.0-1-any.pkg.tar.zst";
  sha256 = "0s4zwj4cwzql5l7yx3rj6c8s9jkhjvqqfv5rg0a2grp4abcmv51m";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-cmake-3.24.0-1-any.pkg.tar.zst";
  sha256 = "0aykg8g07jnsf549ws293ykgsxy2czbnv2yjix1dwilwc9a11w86";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
  sha256 = "0cpyacmciyzbsar1aka5y592g2gpa4i6a58j3bjdmfjdnpm0j08a";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ncurses-6.3-5-any.pkg.tar.zst";
  sha256 = "029z63bw9pwhamw1zi75fr112pxk934nh08by2l54lwdais0vjq8";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-termcap-1.3.1-6-any.pkg.tar.zst";
  sha256 = "1wgbzj53vmv1vm3igjan635j5ims4x19s2y6mgvvc46zgndc2bvq";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-readline-8.1.002-2-any.pkg.tar.zst";
  sha256 = "136fp0cymxqzgs4s8dmal1f4v6ns2mw8jn4cbfihxqb2cmf9yil8";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tcl-8.6.11-5-any.pkg.tar.zst";
  sha256 = "0f3p6x55d0370khpp77xpr1dwhfhrlb8b1wjxxb96y0x67q1casm";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-sqlite3-3.39.1-1-any.pkg.tar.zst";
  sha256 = "0nabw7iy5za5hdpflkgn1s1v93786h9zz6sxzjxm23wymfk1yxlg";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tk-8.6.11.1-2-any.pkg.tar.zst";
  sha256 = "0awr7hzxliyvrkh0ywrga69lcnl5g41i7d4w4azhdwk7i60i1s40";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tzdata-2022a-1-any.pkg.tar.zst";
  sha256 = "0z1q4359q5vfs77a9wnhmf2i9y3ldfmpijjgzqv4za1grmyj6whd";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-3.10.5-3-any.pkg.tar.zst";
  sha256 = "1198p71k30c6kspi8mx6kmsk48fdblfr75291s0gmbmdgba7gfw4";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libgfortran-12.1.0-3-any.pkg.tar.zst";
  sha256 = "11mawrmxp4habwsvbmfsalb136m4dmzlrjy3pcwp7rq8wxx2vnah";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openblas-0.3.20-3-any.pkg.tar.zst";
  sha256 = "07d8cp8in2nbh6dsyis9cvy83y16gz5wfq5fp0fddgh1ak8ihyn2";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-numpy-1.23.1-1-any.pkg.tar.zst";
  sha256 = "05by2nm402jkvzaxcz7g3vmh93qmh6f2ddhambmpn4778np6n9bz";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-setuptools-63.2.0-2-any.pkg.tar.zst";
  sha256 = "0280dajh9rvvg3zl4qrgbap6i6n3lxn172kscn6728ifhn3ap3bh";
})
]
