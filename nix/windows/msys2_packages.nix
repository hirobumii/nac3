{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libwinpthread-git-10.0.0.r258.g530c58e17-1-any.pkg.tar.zst";
  sha256 = "1p2dcs8m50r3lk6myi1y79hmbwsyqr9r8v6k9gl0xa9vda4m7a5s";
  name = "mingw-w64-x86_64-libwinpthread-git-10.0.0.r258.g530c58e17-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libs-12.2.0-11-any.pkg.tar.zst";
  sha256 = "17zdr9vmbv6pkx7k4p5ga622am5fn23jxi275jd8wijbkrwzcvz9";
  name = "mingw-w64-x86_64-gcc-libs-12.2.0-11-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
  sha256 = "19jr14l5anl1qr1lvcmdpnpgyxghf2mds2j7iiq4j99kfg7ig2s0";
  name = "mingw-w64-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-binutils-2.40-2-any.pkg.tar.zst";
  sha256 = "1xa3ni7qg9wzlr903lsqgqisdyvnl28kb3wz2kva21l9i7wwbs7c";
  name = "mingw-w64-x86_64-binutils-2.40-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-headers-git-10.0.0.r258.g530c58e17-1-any.pkg.tar.zst";
  sha256 = "0lxw9hvdniczz5k24ipjvbwcc9p35b97nkn69vb7k8rdp3l95klf";
  name = "mingw-w64-x86_64-headers-git-10.0.0.r258.g530c58e17-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-crt-git-10.0.0.r258.g530c58e17-2-any.pkg.tar.zst";
  sha256 = "12rwwc41as55wqjsi143wfb4722s1zjqzibs05a0gj9c23mivc3q";
  name = "mingw-w64-x86_64-crt-git-10.0.0.r258.g530c58e17-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gmp-6.2.1-5-any.pkg.tar.zst";
  sha256 = "1v19jx0hrsib7ak5jzbhss7p5zjg9y4kj06bcs8sakqvmyby8mlq";
  name = "mingw-w64-x86_64-gmp-6.2.1-5-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-mpfr-4.2.0.p4-1-any.pkg.tar.zst";
  sha256 = "1w46d3d0hvnlw7zc7r3wv7lx8ik8m12hm9pi7k6mbqx1387ljahs";
  name = "mingw-w64-x86_64-mpfr-4.2.0.p4-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-winpthreads-git-10.0.0.r258.g530c58e17-1-any.pkg.tar.zst";
  sha256 = "19h7z6svwnfpyhqh5zjz48jwd2nkvnbj1hbvvs524gdjpi3nxprz";
  name = "mingw-w64-x86_64-winpthreads-git-10.0.0.r258.g530c58e17-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-zlib-1.2.13-3-any.pkg.tar.zst";
  sha256 = "19r9hf111zb41i7r45ixsx26l4sk8g8apryv1xgj03hq54barikz";
  name = "mingw-w64-x86_64-zlib-1.2.13-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-12.2.0-11-any.pkg.tar.zst";
  sha256 = "05lalmfq62jj8nzi7gwxc4zspvcgvif2r2n90ivbfmz2rnxfysml";
  name = "mingw-w64-x86_64-gcc-12.2.0-11-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-p11-kit-0.24.1-5-any.pkg.tar.zst";
  sha256 = "15b11xhn0njgcc3hz6fqfbrqdbir0cbd8fypq7c5il46l9zw90lv";
  name = "mingw-w64-x86_64-p11-kit-0.24.1-5-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-ca-certificates-20230311-1-any.pkg.tar.zst";
  sha256 = "1rcxxlpmvian8c2d9bmnx5hldzvzz4wybz5wp9pjryps1rmzylyx";
  name = "mingw-w64-x86_64-ca-certificates-20230311-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openssl-3.1.0-1-any.pkg.tar.zst";
  sha256 = "01niwg58fass37im83jj1msdhm7kfwnyhvgsgmvh7qhpvfgzfmxl";
  name = "mingw-w64-x86_64-openssl-3.1.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-curl-8.0.1-1-any.pkg.tar.zst";
  sha256 = "0wm2hi51yb5l7v8ds6ypvf0d7knal2r9008a8x38aydja7vyh058";
  name = "mingw-w64-x86_64-curl-8.0.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-xz-5.4.2-1-any.pkg.tar.zst";
  sha256 = "0w63y7rg76jnzy7ijhda1jzi9jgy1sdfvzq32zyj8yibkcpfxlll";
  name = "mingw-w64-x86_64-xz-5.4.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-libxml2-2.10.4-1-any.pkg.tar.zst";
  sha256 = "0baqj107v7y51wrj9zpjw1s146wwdfx2gwjzp7kil82yr9qbw9lk";
  name = "mingw-w64-x86_64-libxml2-2.10.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-rust-1.69.0-1-any.pkg.tar.zst";
  sha256 = "1ff32b6hrgqmm26y9i8hwcs5w3cr1kvdq3sas2jaxzq8bfyyff1w";
  name = "mingw-w64-x86_64-rust-1.69.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-cmake-3.26.3-2-any.pkg.tar.zst";
  sha256 = "1qjrwd4n7v9nakxvp1j6qdqpya8k42aj9vj1cdk3j2d501kwpwrj";
  name = "mingw-w64-x86_64-cmake-3.26.3-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tcl-8.6.12-2-any.pkg.tar.zst";
  sha256 = "17lyrkyh76lh0ghk19d91kwicms6lxshhhb6n3zh748awfvihknm";
  name = "mingw-w64-x86_64-tcl-8.6.12-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-sqlite3-3.41.2-1-any.pkg.tar.zst";
  sha256 = "0s1kzipdz7rczpk32wkbc90yjw6wbcxmzlgnd7j99fzwaawblbnp";
  name = "mingw-w64-x86_64-sqlite3-3.41.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tk-8.6.12-1-any.pkg.tar.zst";
  sha256 = "1pnznf4a195ij3b1g921k0llkn62wf0piijldj2c7qlbcq73v66c";
  name = "mingw-w64-x86_64-tk-8.6.12-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-tzdata-2023c-1-any.pkg.tar.zst";
  sha256 = "0ifavpqi1ykn9962ic4sh5l18y2mvz9pj6742fgw85s9wixbj7fl";
  name = "mingw-w64-x86_64-tzdata-2023c-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-3.10.11-1-any.pkg.tar.zst";
  sha256 = "0fl3jrmp59180is4l9d8qimslnpqrzvl7wprqjhag2c7ki9c0gg4";
  name = "mingw-w64-x86_64-python-3.10.11-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-gcc-libgfortran-12.2.0-11-any.pkg.tar.zst";
  sha256 = "14k4mqq2w7jgxq5xhaj14na6vc237v4jp9f600c04j8w8g28j073";
  name = "mingw-w64-x86_64-gcc-libgfortran-12.2.0-11-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-openblas-0.3.23-1-any.pkg.tar.zst";
  sha256 = "1zqxcpmw3mbw1axsf90sk7vmswq95g1b3pj826zrf42bjcjbsjpk";
  name = "mingw-w64-x86_64-openblas-0.3.23-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-numpy-1.24.2-3-any.pkg.tar.zst";
  sha256 = "0d81782q72ygkycfn1y2yvsp7g55190zdklkv42pizzapn8h3a2l";
  name = "mingw-w64-x86_64-python-numpy-1.24.2-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/mingw64/mingw-w64-x86_64-python-setuptools-67.7.1-1-any.pkg.tar.zst";
  sha256 = "0d4j7n133q4whs8ipjqn9azqfc1bx60dylmjwgq62vn3fxbf307p";
  name = "mingw-w64-x86_64-python-setuptools-67.7.1-1-any.pkg.tar.zst";
})
]
