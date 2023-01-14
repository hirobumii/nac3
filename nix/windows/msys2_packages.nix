{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libiconv-1.17-1-any.pkg.tar.zst";
  sha256 = "1pb1x5wrlmmpjdpzsc7rs5xk6ydlsd5mval0fwrqq54jf6dxdzpz";
  name = "mingw-w64-x86_64-libiconv-1.17-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zlib-1.2.13-2-any.pkg.tar.zst";
  sha256 = "0v2hkq7yjyq3s0iknnd27qrpxl51g6ks5dv7mjn44cnwplqibnc6";
  name = "mingw-w64-x86_64-zlib-1.2.13-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-binutils-2.39-2-any.pkg.tar.zst";
  sha256 = "15swxdp3zwqs9wvbqrc0fmchd1797qd81r7ipq3sqrrmf4bmq50g";
  name = "mingw-w64-x86_64-binutils-2.39-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-headers-git-10.0.0.r202.g4359b3570-1-any.pkg.tar.zst";
  sha256 = "050vvkfcsk1r6lvai8ppc6jpyvyiv5v04ps04pgb11zs8kh94s3s";
  name = "mingw-w64-x86_64-headers-git-10.0.0.r202.g4359b3570-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-crt-git-10.0.0.r202.g4359b3570-1-any.pkg.tar.zst";
  sha256 = "1z573mzjdiswizq147cagvb7xwza0qb99n7vyfncyidp1anvcgwy";
  name = "mingw-w64-x86_64-crt-git-10.0.0.r202.g4359b3570-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gmp-6.2.1-4-any.pkg.tar.zst";
  sha256 = "1ly6vykj87sr6l6dj986zhn5mskgjj4gv81dmz19m5vq73z56xgz";
  name = "mingw-w64-x86_64-gmp-6.2.1-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-isl-0.25-1-any.pkg.tar.zst";
  sha256 = "0hky9gmd6iz1s3irmp9fk2j10cpqrrw8l810riwr58ynj3i10j2k";
  name = "mingw-w64-x86_64-isl-0.25-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libwinpthread-git-10.0.0.r202.g4359b3570-1-any.pkg.tar.zst";
  sha256 = "00q62a3d8sdffbsykprcg1ivxj4nyr6wlznc0vlq7jw30368vmcb";
  name = "mingw-w64-x86_64-libwinpthread-git-10.0.0.r202.g4359b3570-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libs-12.2.0-7-any.pkg.tar.zst";
  sha256 = "0hb04rr0maamv0f9ns8c4w3w31aa2akvvc9ab8n3qp4hbcy1x7s6";
  name = "mingw-w64-x86_64-gcc-libs-12.2.0-7-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-winpthreads-git-10.0.0.r202.g4359b3570-1-any.pkg.tar.zst";
  sha256 = "1ljkrfv1hgzdl6g60yb1b4zpdjcc8xj379xblrkvhfgj9y8pyi8c";
  name = "mingw-w64-x86_64-winpthreads-git-10.0.0.r202.g4359b3570-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zstd-1.5.2-2-any.pkg.tar.zst";
  sha256 = "1f14wbc1yvjgv3rbwhv75391l55gcm0as6ipba20vw8phz4ax8ds";
  name = "mingw-w64-x86_64-zstd-1.5.2-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-12.2.0-7-any.pkg.tar.zst";
  sha256 = "1n486mr6c2xhmn4yhv5xyapz7f7l2lajgbr1b3prc0yx1h18dwkv";
  name = "mingw-w64-x86_64-gcc-12.2.0-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-c-ares-1.18.1-2-any.pkg.tar.zst";
  sha256 = "1bx3x4xqsv6afdkq7as7pjcdpnhalr0lqsxg9ryx2g84iq4jy6za";
  name = "mingw-w64-x86_64-c-ares-1.18.1-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gettext-0.21-3-any.pkg.tar.zst";
  sha256 = "1gy7fmn6jc13ipnyyq44gyhv8rvz5cy7gz1dm3wrna80hjnzli5v";
  name = "mingw-w64-x86_64-gettext-0.21-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libunistring-1.0-1-any.pkg.tar.zst";
  sha256 = "1qks1gm8jscnn93sr7n1azkzcq4a8fybsikpqcf920m9b66cym4k";
  name = "mingw-w64-x86_64-libunistring-1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libidn2-2.3.3-1-any.pkg.tar.zst";
  sha256 = "1m3qgnhgf0g389kglrai26x4k64gs2cy9b3mjwlkw5xcs2r3smww";
  name = "mingw-w64-x86_64-libidn2-2.3.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libpsl-0.21.2-3-any.pkg.tar.zst";
  sha256 = "1hywa9qbcncb64p6x4kmm7ffm4l37p47yln9h0r489av665wqpr5";
  name = "mingw-w64-x86_64-libpsl-0.21.2-3-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ca-certificates-20211016-3-any.pkg.tar.zst";
  sha256 = "02x6dnbbyjm6mcl6ii61bc5rkwg3qsbaqd2lyzsp5732hxjcmmq4";
  name = "mingw-w64-x86_64-ca-certificates-20211016-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openssl-1.1.1.s-1-any.pkg.tar.zst";
  sha256 = "0bwjrsnn54kjq2gxvmcyrngk84347pvyd6hfwq4mzxz18z15r3dx";
  name = "mingw-w64-x86_64-openssl-1.1.1.s-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libssh2-1.10.0-1-any.pkg.tar.zst";
  sha256 = "1f27an41hxrfs9jifq0708c484ps3zmb582gmsy7xn5idg3wk03d";
  name = "mingw-w64-x86_64-libssh2-1.10.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-nghttp2-1.51.0-1-any.pkg.tar.zst";
  sha256 = "077gj6y04jri9nfy77n8vncppk86yngx1cli0gb2a6bg4amzpgfk";
  name = "mingw-w64-x86_64-nghttp2-1.51.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-curl-7.87.0-1-any.pkg.tar.zst";
  sha256 = "0y4mwsl79z2a7djcgrpjm6bvs84wzhs1kfmdvqsgfdy1c0ryalcp";
  name = "mingw-w64-x86_64-curl-7.87.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-xz-5.2.9-1-any.pkg.tar.zst";
  sha256 = "1aiv10aldz9gq0yzcm36sf46h84hgm7012dacs90b5l8axk86pwn";
  name = "mingw-w64-x86_64-xz-5.2.9-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libxml2-2.10.3-1-any.pkg.tar.zst";
  sha256 = "087835d8lg19drq9wcn9fpbdvai0pcsh6layhvd9zh67bgpgyaq9";
  name = "mingw-w64-x86_64-libxml2-2.10.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rust-1.66.1-1-any.pkg.tar.zst";
  sha256 = "1fn2rvgrly0r9zj4275pbrvz1ilrp4j2l3sazv8pwz4l26ddyldi";
  name = "mingw-w64-x86_64-rust-1.66.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libarchive-3.6.2-1-any.pkg.tar.zst";
  sha256 = "1x44xbh2sbqjn68iywb28qgflkpps51l07wqnavs7yanw4svkslb";
  name = "mingw-w64-x86_64-libarchive-3.6.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libuv-1.44.2-2-any.pkg.tar.zst";
  sha256 = "143qq7373x4zpha1nksa7ah7hxz0qirgdj1s09pb3hcap1ijbjp2";
  name = "mingw-w64-x86_64-libuv-1.44.2-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rhash-1.4.3-1-any.pkg.tar.zst";
  sha256 = "1nd0iqlx1vmn079i24i07r4kqfr3yr0apnzsgcx8qd5cyvwnl7w6";
  name = "mingw-w64-x86_64-rhash-1.4.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ninja-1.11.1-2-any.pkg.tar.zst";
  sha256 = "0d0hxjqgfwrh4rz7apgjfdirjph18zc55amr01903ifd1kwsvsbr";
  name = "mingw-w64-x86_64-ninja-1.11.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-cmake-3.25.1-2-any.pkg.tar.zst";
  sha256 = "1n9bp8i4k1948nfjbhdscm8czfkqhgjc1s863rnyiifcbkfb86h2";
  name = "mingw-w64-x86_64-cmake-3.25.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
  sha256 = "0cpyacmciyzbsar1aka5y592g2gpa4i6a58j3bjdmfjdnpm0j08a";
  name = "mingw-w64-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ncurses-6.3-6-any.pkg.tar.zst";
  sha256 = "1847q7ydrbkvfzrkyywph5lh1kgj44mqchmhjsmafis86m2rswib";
  name = "mingw-w64-x86_64-ncurses-6.3-6-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-sqlite3-3.40.1-1-any.pkg.tar.zst";
  sha256 = "1knsg7my1z0n0d96yn6n50mk40z5493yi7lzyx8rbx8d5wvzagwr";
  name = "mingw-w64-x86_64-sqlite3-3.40.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-3.10.9-1-any.pkg.tar.zst";
  sha256 = "1vbmgdjyhkll64v8nn6ajzd03q1nxp9wav934z27jmrbjp9z2xkh";
  name = "mingw-w64-x86_64-python-3.10.9-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libgfortran-12.2.0-7-any.pkg.tar.zst";
  sha256 = "18fbcn8rcnryhy0f5r2gyyk62x4ycv5m4kbhqllvxj7g9d521mhp";
  name = "mingw-w64-x86_64-gcc-libgfortran-12.2.0-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openblas-0.3.21-7-any.pkg.tar.zst";
  sha256 = "09pr50afm2xbbalvfxv5vvaf48sckacb6qwr88dghyjkiqk0wds8";
  name = "mingw-w64-x86_64-openblas-0.3.21-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-numpy-1.23.5-1-any.pkg.tar.zst";
  sha256 = "1kh381jzjbw4nxriqcmi8phs6fc80im48dg0cyqrkf64rqwgkj5w";
  name = "mingw-w64-x86_64-python-numpy-1.23.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-setuptools-65.6.3-1-any.pkg.tar.zst";
  sha256 = "1gnvm13j5cw8r9kpx1sy45ygmml3ywz3nprsqrg2z0bpjyl3whq8";
  name = "mingw-w64-x86_64-python-setuptools-65.6.3-1-any.pkg.tar.zst";
})
]
