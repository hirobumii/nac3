{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.4.4-1-any.pkg.tar.zst";
  sha256 = "0mws1g7w11riczc168x7kzb8nl74iry4bzkb72nspw1vmlblxfy6";
  name = "mingw-w64-clang-x86_64-libffi-3.4.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-17.0.4-1-any.pkg.tar.zst";
  sha256 = "1748ncih1zj4vnj9c0gcp5rf21gm2pn9xdy2hrigp2pvfchrnmwn";
  name = "mingw-w64-clang-x86_64-libunwind-17.0.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-17.0.4-1-any.pkg.tar.zst";
  sha256 = "00waw5qmpycib7xhk6aywqgqg8y26q2xbm8m4za7mpy04g2alav4";
  name = "mingw-w64-clang-x86_64-libc++-17.0.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3-1-any.pkg.tar.zst";
  sha256 = "0rfwz7czvwa8clickjw1kd114miyifxf1s2v9mxffkaivm45f62v";
  name = "mingw-w64-clang-x86_64-zlib-1.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libiconv-1.17-3-any.pkg.tar.zst";
  sha256 = "1hxmdgivb86h7wz9hcp0had99ngv157w1fbjg7cgy068zv787m8w";
  name = "mingw-w64-clang-x86_64-libiconv-1.17-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.5.0-1-any.pkg.tar.zst";
  sha256 = "1gi9ckh48k64gras307f6pf5y558hj80izlxri8cnwvzzmmra8dg";
  name = "mingw-w64-clang-x86_64-expat-2.5.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gettext-0.22.4-3-any.pkg.tar.zst";
  sha256 = "0i2gbhzvcy8cq7gnd9zsjw47jmn8gsq5pam5j3yn84jn578f3mfi";
  name = "mingw-w64-clang-x86_64-gettext-0.22.4-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.4.5-1-any.pkg.tar.zst";
  sha256 = "1niky9s7qq0434ljma3f9m8yybcifkvkxwhk580crzb2ifpmqxc3";
  name = "mingw-w64-clang-x86_64-xz-5.4.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.12.1-1-any.pkg.tar.zst";
  sha256 = "0lajdhkqv3hrfmnf9wa0ddqpr9z8rvb037rv0ss60g2n0b92s5gk";
  name = "mingw-w64-clang-x86_64-libxml2-2.12.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
  sha256 = "07739wmwgxf0d6db4p8w302a6jwcm01aafr1s8jvcl5k1h5a1m2m";
  name = "mingw-w64-clang-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-17.0.4-1-any.pkg.tar.zst";
  sha256 = "13gpp9ah8g4ggzmgsskwcrck16rvjwp464i5vyw9zwm88bdhmz93";
  name = "mingw-w64-clang-x86_64-llvm-libs-17.0.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-17.0.4-1-any.pkg.tar.zst";
  sha256 = "103zfg4ypmzhy0gd02arjydca2l4ak2mai3g1xrck83l15v85f9h";
  name = "mingw-w64-clang-x86_64-llvm-17.0.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-17.0.4-1-any.pkg.tar.zst";
  sha256 = "0n38jb9gl3zmy7f07bqzpk70k00hdrnasrclhcxgbn4ylzlca07w";
  name = "mingw-w64-clang-x86_64-compiler-rt-17.0.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-11.0.0.r404.g3a137bd87-1-any.pkg.tar.zst";
  sha256 = "1wisa5j86xn8fanx4ks5pkj2hx0k89cippcap6c6yf6d3qmj4mvr";
  name = "mingw-w64-clang-x86_64-headers-git-11.0.0.r404.g3a137bd87-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-11.0.0.r404.g3a137bd87-1-any.pkg.tar.zst";
  sha256 = "0x1x3rnxivm58sqjx5v5smnzl5hdinx4qrjgkn59nylcd24fvnk4";
  name = "mingw-w64-clang-x86_64-crt-git-11.0.0.r404.g3a137bd87-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-17.0.4-1-any.pkg.tar.zst";
  sha256 = "1028iymsay6smk856xfyccl7pkcxzigqlrhj4sgz4xxqij7xqnp0";
  name = "mingw-w64-clang-x86_64-lld-17.0.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-11.0.0.r404.g3a137bd87-1-any.pkg.tar.zst";
  sha256 = "01q7r8mvn60yc0bxlhfbb1rx8w6lbvf1bd0xfaycqmc25b8vnfnp";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-11.0.0.r404.g3a137bd87-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-11.0.0.r404.g3a137bd87-1-any.pkg.tar.zst";
  sha256 = "0f5828injyxhy8vxkv02wmk4fh6x5gsqnkr97p50vz692fxkwk48";
  name = "mingw-w64-clang-x86_64-winpthreads-git-11.0.0.r404.g3a137bd87-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-17.0.4-1-any.pkg.tar.zst";
  sha256 = "1adfijlky02waz6gm2rx32rd413ws5rphkpybihpc8hi11yag761";
  name = "mingw-w64-clang-x86_64-clang-17.0.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.22.1-1-any.pkg.tar.zst";
  sha256 = "0bv8n3862krxhlmz0lxqq71y40dy4flpjvb7adl5a96jixgsbxav";
  name = "mingw-w64-clang-x86_64-c-ares-1.22.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-brotli-1.1.0-1-any.pkg.tar.zst";
  sha256 = "113mha41q53cx0hw13cq1xdf7zbsd58sh8cl1cd7xzg1q69n60w2";
  name = "mingw-w64-clang-x86_64-brotli-1.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunistring-1.1-1-any.pkg.tar.zst";
  sha256 = "16myvbg33q5s7jl30w5qd8n8f1r05335ms8r61234vn52n32l2c4";
  name = "mingw-w64-clang-x86_64-libunistring-1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libidn2-2.3.4-1-any.pkg.tar.zst";
  sha256 = "105valrldri39sx7d5zdscxgsz9px382f8vbbl2zpr2xzb3jq8p8";
  name = "mingw-w64-clang-x86_64-libidn2-2.3.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libpsl-0.21.2-4-any.pkg.tar.zst";
  sha256 = "0h4inq6prhiipl3h5k9br9rz5mih123b8n12wiwp2qck0h2q3x98";
  name = "mingw-w64-clang-x86_64-libpsl-0.21.2-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtasn1-4.19.0-1-any.pkg.tar.zst";
  sha256 = "19m59mjxww26ah2gk9c0i512fmqpyaj6r5na564kmg6wpwvkihcj";
  name = "mingw-w64-clang-x86_64-libtasn1-4.19.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-p11-kit-0.25.3-1-any.pkg.tar.zst";
  sha256 = "19330k2jxpzm552m7j116hz8qrn2d14h9ybi0lcmkj4c1l25m6mf";
  name = "mingw-w64-clang-x86_64-p11-kit-0.25.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20230311-1-any.pkg.tar.zst";
  sha256 = "00hdl239695xi5bgld7a1ssp6kapkb9az02dpx80vmz7mqg6wwxx";
  name = "mingw-w64-clang-x86_64-ca-certificates-20230311-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.2.0-1-any.pkg.tar.zst";
  sha256 = "1623bkf4bjwf4cs1j5f6c37gwj4z775kjk4jswwy58r61yimwnd5";
  name = "mingw-w64-clang-x86_64-openssl-3.2.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
  sha256 = "0l2m823gm1rvnjmqm5ads17mxz1bhpzai5ixyhnkpzrsjxd1ygy5";
  name = "mingw-w64-clang-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.58.0-1-any.pkg.tar.zst";
  sha256 = "00f7raqmky43v9h2356yzfbvbhkbsjpc17n6ksgpv66h31svna6q";
  name = "mingw-w64-clang-x86_64-nghttp2-1.58.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.4.0-2-any.pkg.tar.zst";
  sha256 = "1qi446lq1qqq45xn7fmasfb914jhahqc0cvpnqi8vrp9vlk9dsw3";
  name = "mingw-w64-clang-x86_64-curl-8.4.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.74.0-1-any.pkg.tar.zst";
  sha256 = "071xpmfsnr7j7ycf6w8kyg41fxs3dclnjrcp7grinck5bb5zxn6s";
  name = "mingw-w64-clang-x86_64-rust-1.74.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pkgconf-1~2.1.0-1-any.pkg.tar.zst";
  sha256 = "04y364mzx2sj984cdxq187fg73hzkrzvbpsr5d86xfzrag789nh3";
  name = "mingw-w64-clang-x86_64-pkgconf-12.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-jsoncpp-1.9.5-2-any.pkg.tar.zst";
  sha256 = "0cpy76crngj5dfg9f4l216ry0wcavp0nabyc0b9g676rg6400qas";
  name = "mingw-w64-clang-x86_64-jsoncpp-1.9.5-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-bzip2-1.0.8-3-any.pkg.tar.zst";
  sha256 = "1n8zf2kk1xj7wiszp6mjchy1yzpalddbj0cj17qm625ags2vzflm";
  name = "mingw-w64-clang-x86_64-bzip2-1.0.8-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libb2-0.98.1-2-any.pkg.tar.zst";
  sha256 = "0555dvb2xs6695sz5ndrx6y0cz3qa5cg0m5v8q1md13ssg76vlh6";
  name = "mingw-w64-clang-x86_64-libb2-0.98.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lz4-1.9.4-1-any.pkg.tar.zst";
  sha256 = "0nn7cy25j53q5ckkx4n4f77w00xdwwf5wjswm374shvvs58nlln0";
  name = "mingw-w64-clang-x86_64-lz4-1.9.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtre-git-r128.6fb7206-2-any.pkg.tar.zst";
  sha256 = "0bmdla75k0q88l93ql9ajbfag4vhdhyp0glzymljvcc7ir0r6f9r";
  name = "mingw-w64-clang-x86_64-libtre-git-r128.6fb7206-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libsystre-1.0.1-4-any.pkg.tar.zst";
  sha256 = "0x5ns8ld08gd9r5a98sqi3sm0vz588caaqw10ciix16sgyisfpdh";
  name = "mingw-w64-clang-x86_64-libsystre-1.0.1-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.7.2-1-any.pkg.tar.zst";
  sha256 = "1p84yh6yzkdpmr02vyvgz16x5gycckah25jkdc2py09l7iw96bmw";
  name = "mingw-w64-clang-x86_64-libarchive-3.7.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.47.0-1-any.pkg.tar.zst";
  sha256 = "1ch8g0mp13dmwi7sc6qxjcx18s1w0bsdl5lhkjmx5ccz55sspnyf";
  name = "mingw-w64-clang-x86_64-libuv-1.47.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ninja-1.11.1-3-any.pkg.tar.zst";
  sha256 = "13wjfmyfr952n3ydpldjlwx1nla5xpyvr96ng8pfbyw4z900v5ms";
  name = "mingw-w64-clang-x86_64-ninja-1.11.1-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.3-1-any.pkg.tar.zst";
  sha256 = "16ghg6894gb3lcrcpc8g1jd7524djnsjrqcr3krqlzskfv51hgj6";
  name = "mingw-w64-clang-x86_64-rhash-1.4.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.27.8-1-any.pkg.tar.zst";
  sha256 = "1hm5975q0kfd0z502qpxq1imwvby3qb93jzhbvrg2fz9zf5d31rg";
  name = "mingw-w64-clang-x86_64-cmake-3.27.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
  sha256 = "0zm9pqcgjk9a8ql6hxcxh477wvvyimndcisd6zrr6y8m5vvklsdi";
  name = "mingw-w64-clang-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.4.20230708-1-any.pkg.tar.zst";
  sha256 = "0l960cf4m558kx6dwahl9z2zdb9vlqd7qgk39kg924mzrfjrsvv5";
  name = "mingw-w64-clang-x86_64-ncurses-6.4.20230708-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
  sha256 = "17ha468qavwin800cc3b7c3xdggwk2gakasfxg7jdx7616d99l0n";
  name = "mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-readline-8.2.001-6-any.pkg.tar.zst";
  sha256 = "1aw1qxjvifkzxnn52l6hba1kcj2dci8llzzj29zbvbq5hgllf6dv";
  name = "mingw-w64-clang-x86_64-readline-8.2.001-6-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tcl-8.6.12-2-any.pkg.tar.zst";
  sha256 = "06r7ni7m1vf83ms8ha3glalc5rfriiffh7v644jmnvzs5g9x0pzb";
  name = "mingw-w64-clang-x86_64-tcl-8.6.12-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.44.0-1-any.pkg.tar.zst";
  sha256 = "022vy54ph6kxmgafbsmgpclvd55ljarvc4k7497ka03g7465zq6j";
  name = "mingw-w64-clang-x86_64-sqlite3-3.44.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tk-8.6.12-2-any.pkg.tar.zst";
  sha256 = "0pi74q91vl6vw8vvmmwnvrgai3b1aanp0zhca5qsmv8ljh2wdgzx";
  name = "mingw-w64-clang-x86_64-tk-8.6.12-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2023c-1-any.pkg.tar.zst";
  sha256 = "018qw8lk5r7gqvavnl1414gmbd4bx22528a7bjk4injfr3cib9c2";
  name = "mingw-w64-clang-x86_64-tzdata-2023c-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.11.6-2-any.pkg.tar.zst";
  sha256 = "0j6wkjjay52f4mgvfagi7xql4v74dizhmwdx07n2fixr7j1lg6n7";
  name = "mingw-w64-clang-x86_64-python-3.11.6-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openmp-17.0.4-1-any.pkg.tar.zst";
  sha256 = "1y2cf43fcyb1qv4kn0h0mzljmibaqklcj167krwxq5ymfbipy1yw";
  name = "mingw-w64-clang-x86_64-openmp-17.0.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.25-1-any.pkg.tar.zst";
  sha256 = "1789hd7sc249vbzppqqg56i080c8kc6bj6qngn9rjvzcdllh0d07";
  name = "mingw-w64-clang-x86_64-openblas-0.3.25-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-1.26.2-1-any.pkg.tar.zst";
  sha256 = "1njbxlxha05c8fzm7yl3ksjlpanh5jxn1z1s06iv9knx2zmqjk6f";
  name = "mingw-w64-clang-x86_64-python-numpy-1.26.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-68.2.2-1-any.pkg.tar.zst";
  sha256 = "16m1ph7yq3mqzaaaka5vij27jzaw59lpxwbmpdxkcp8lb6h5xcn9";
  name = "mingw-w64-clang-x86_64-python-setuptools-68.2.2-1-any.pkg.tar.zst";
})
]
