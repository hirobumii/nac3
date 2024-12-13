{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-19.1.4-1-any.pkg.tar.zst";
  sha256 = "0frb5k16bbxdf8g379d16vl3qrh7n9pydn83gpfxpvwf3qlvnzyl";
  name = "mingw-w64-clang-x86_64-libunwind-19.1.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-19.1.4-1-any.pkg.tar.zst";
  sha256 = "0wh5km0v8j50pqz9bxb4f0w7r8zhsvssrjvc94np53iq8wjagk86";
  name = "mingw-w64-clang-x86_64-libc++-19.1.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.4.6-1-any.pkg.tar.zst";
  sha256 = "1q6gms980985bp087rnnpvz2fwfakgm5266izfk3b1mbp620s1yv";
  name = "mingw-w64-clang-x86_64-libffi-3.4.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libiconv-1.17-4-any.pkg.tar.zst";
  sha256 = "1g2bkhgf60dywccxw911ydyigf3m25yqfh81m5099swr7mjsmzyf";
  name = "mingw-w64-clang-x86_64-libiconv-1.17-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gettext-runtime-0.22.5-2-any.pkg.tar.zst";
  sha256 = "0ll6ci6d3mc7g04q0xixjc209bh8r874dqbczgns69jsad3wg6mi";
  name = "mingw-w64-clang-x86_64-gettext-runtime-0.22.5-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.6.3-3-any.pkg.tar.zst";
  sha256 = "1a7gc462gnrjy5qb0zfkr9qm8bsnnf02y6wp3c59n618dhsq7rcf";
  name = "mingw-w64-clang-x86_64-xz-5.6.3-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
  sha256 = "06i9xjsskf4ddb2ph4h31md5c7imj9mzjhd4lc4q44j8dmpc1w5p";
  name = "mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.12.9-2-any.pkg.tar.zst";
  sha256 = "1b1r5llgqv88id8iwhqh23qwqmn5ic9hdamdc8xzij9hmcvdmmci";
  name = "mingw-w64-clang-x86_64-libxml2-2.12.9-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.6-2-any.pkg.tar.zst";
  sha256 = "02cp5ci8w50k7xn38mpkwnr8sn898v18wcc07y8f9sfla7vcyfix";
  name = "mingw-w64-clang-x86_64-zstd-1.5.6-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-19.1.4-1-any.pkg.tar.zst";
  sha256 = "1clrbm8dk893byj8s15pgcgqqijm2zkd10zgyakamd8m354kj9q4";
  name = "mingw-w64-clang-x86_64-llvm-libs-19.1.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-19.1.4-1-any.pkg.tar.zst";
  sha256 = "1iz2c9475h8p20ydpp0znbhyb62rlrk7wr7xl7cmwbam7wkwr8rn";
  name = "mingw-w64-clang-x86_64-llvm-19.1.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-19.1.4-1-any.pkg.tar.zst";
  sha256 = "1hidciwlakxrp4kyb0j2v6g4lv76nn834g6b88w1j94fk3qc765d";
  name = "mingw-w64-clang-x86_64-clang-libs-19.1.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-19.1.4-1-any.pkg.tar.zst";
  sha256 = "1m1yhjkgzlbk10sv966qk4yji009ga0lr25gpgj2w7mcd2wixcr3";
  name = "mingw-w64-clang-x86_64-compiler-rt-19.1.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-12.0.0.r423.g8bcd5fc1a-1-any.pkg.tar.zst";
  sha256 = "08gxc7h2achckknn6fz3p6yi7gxxvbaday8fpm4j56c4sa04n0df";
  name = "mingw-w64-clang-x86_64-headers-git-12.0.0.r423.g8bcd5fc1a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-12.0.0.r423.g8bcd5fc1a-1-any.pkg.tar.zst";
  sha256 = "0fxd1pb197ki0gzw6z8gmd6wgpd9d28js6cp5d31d55kw7d1vz13";
  name = "mingw-w64-clang-x86_64-crt-git-12.0.0.r423.g8bcd5fc1a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-19.1.4-1-any.pkg.tar.zst";
  sha256 = "1a8pjyhrzpc2z3784xxwix4i7yrz03ygnsk1wv9k0yq8m8wi9nbw";
  name = "mingw-w64-clang-x86_64-lld-19.1.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r423.g8bcd5fc1a-1-any.pkg.tar.zst";
  sha256 = "140m312jx1sywqjkvfij69d268m4jpdmilq5bb8khkf0ayb16036";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r423.g8bcd5fc1a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r423.g8bcd5fc1a-1-any.pkg.tar.zst";
  sha256 = "017j4h511wg37bacym73f8g6s0jcfgzbzabzxpc6anr3gy4kkpbg";
  name = "mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r423.g8bcd5fc1a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-19.1.4-1-any.pkg.tar.zst";
  sha256 = "11f4i4ai2bzvq6f06vxk1ymv7056c9707vdw489f1i2bdrf0c0ii";
  name = "mingw-w64-clang-x86_64-clang-19.1.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.83.0-3-any.pkg.tar.zst";
  sha256 = "0nxs571vb4f1i5vp91134p5blns9ml2r25nx6kdlg0zhd5x85kvm";
  name = "mingw-w64-clang-x86_64-rust-1.83.0-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
  sha256 = "0phhwkcqp30dsyj5vr6w99sgm1jfm5rzg0w5x5mv9md4x7lm9lmh";
  name = "mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.34.3-1-any.pkg.tar.zst";
  sha256 = "1mpn397qsdz3l2fav6ymwjlj96ialn9m8sldii3ymbcyhranl3xx";
  name = "mingw-w64-clang-x86_64-c-ares-1.34.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-brotli-1.1.0-4-any.pkg.tar.zst";
  sha256 = "0hx9gjzibacfx3fzk11n2vzz2pmnb956babh2ig8avx3hk7vlqrg";
  name = "mingw-w64-clang-x86_64-brotli-1.1.0-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunistring-1.2-1-any.pkg.tar.zst";
  sha256 = "13nz49li39z1zgfx1q9jg4vrmyrmqb6qdq0nqshidaqc6zr16k3g";
  name = "mingw-w64-clang-x86_64-libunistring-1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libidn2-2.3.7-2-any.pkg.tar.zst";
  sha256 = "07k8zh5nb2s82md7lz22r8gim8214rhlg586lywck3zcla98jv1w";
  name = "mingw-w64-clang-x86_64-libidn2-2.3.7-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libpsl-0.21.5-3-any.pkg.tar.zst";
  sha256 = "0hb7wgdliic3d7fa0cvr5pj946pmwfc0apmyb0yfb5d0hc1afwsc";
  name = "mingw-w64-clang-x86_64-libpsl-0.21.5-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtasn1-4.19.0-1-any.pkg.tar.zst";
  sha256 = "19m59mjxww26ah2gk9c0i512fmqpyaj6r5na564kmg6wpwvkihcj";
  name = "mingw-w64-clang-x86_64-libtasn1-4.19.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-p11-kit-0.25.5-1-any.pkg.tar.zst";
  sha256 = "00yz6cmr1ldlrskv811n345xcia88mj7w4fyx4m9z5848jxgsabd";
  name = "mingw-w64-clang-x86_64-p11-kit-0.25.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20240203-1-any.pkg.tar.zst";
  sha256 = "1q5nxhsk04gidz66ai5wgd4dr04lfyakkfja9p0r5hrgg4ppqqjg";
  name = "mingw-w64-clang-x86_64-ca-certificates-20240203-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.4.0-1-any.pkg.tar.zst";
  sha256 = "0cgiqjmgdnwnv9r88z634dmqrzh06dmsfncyzymw0s16nnv2k7k2";
  name = "mingw-w64-clang-x86_64-openssl-3.4.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.1-1-any.pkg.tar.zst";
  sha256 = "01l23cn5brficjzba7ldscqkdvk4rdcvvdyybd90qr2hqzligmhn";
  name = "mingw-w64-clang-x86_64-libssh2-1.11.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.64.0-1-any.pkg.tar.zst";
  sha256 = "1hv8fp496l018s5dx5v8nvxc0a6rswskwk1jsrfd94rh3kbq2ilc";
  name = "mingw-w64-clang-x86_64-nghttp2-1.64.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.6.0-1-any.pkg.tar.zst";
  sha256 = "1p7q47fin12vzyf126v1azbbpgpa0y6ighfh6mbfdb6zcyq74kbd";
  name = "mingw-w64-clang-x86_64-nghttp3-1.6.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.11.1-1-any.pkg.tar.zst";
  sha256 = "16yvyqjzxyzawgv26r1g145wphvhjil2b0pyhy4nj7v5d19n6wvh";
  name = "mingw-w64-clang-x86_64-curl-8.11.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.6.4-1-any.pkg.tar.zst";
  sha256 = "03fp2yacv7gk0g049lffz6pbj93vpjmzqxxa312d4gxczi57nqdv";
  name = "mingw-w64-clang-x86_64-expat-2.6.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-jsoncpp-1.9.6-3-any.pkg.tar.zst";
  sha256 = "1ipilhiza17vz5dhgi61l80w2klw9f21w6jbyhi9wmfd6nxqv13c";
  name = "mingw-w64-clang-x86_64-jsoncpp-1.9.6-3-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lz4-1.10.0-1-any.pkg.tar.zst";
  sha256 = "0kznnw9z9zqxkmn8qbypm2rpsfaapbgls1ks3zzpfnfjz9cpw8py";
  name = "mingw-w64-clang-x86_64-lz4-1.10.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtre-0.9.0-1-any.pkg.tar.zst";
  sha256 = "1y5d2cbkpd0ngqlsv6hvz47nxb1wry33i5s1clg2k53yhgjkg8vv";
  name = "mingw-w64-clang-x86_64-libtre-0.9.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libsystre-1.0.1-6-any.pkg.tar.zst";
  sha256 = "19c71fs5gqjrf88mv7l702fjg228xd9lfbxg0mkzm3ljvv4ljn0q";
  name = "mingw-w64-clang-x86_64-libsystre-1.0.1-6-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.7.7-1-any.pkg.tar.zst";
  sha256 = "01glychb0k1yd878aq4y2fn08lqh2bjydh90xmq03z5qhig66mmn";
  name = "mingw-w64-clang-x86_64-libarchive-3.7.7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.49.2-1-any.pkg.tar.zst";
  sha256 = "1b9slshbcprxjaj2qqypaywr0f2pgajg1bgspjk83hk65sx6sklb";
  name = "mingw-w64-clang-x86_64-libuv-1.49.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ninja-1.12.1-1-any.pkg.tar.zst";
  sha256 = "1vj9qaa43v316daz8k4ricmz3f33nhjpj7r0vn979nwmy7hzs7jx";
  name = "mingw-w64-clang-x86_64-ninja-1.12.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pkgconf-1~2.3.0-1-any.pkg.tar.zst";
  sha256 = "15i7x6akkgs7aa7aa804k93p2iipnvygsy7z8hsafskka3h150fa";
  name = "mingw-w64-clang-x86_64-pkgconf-12.3.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.4-3-any.pkg.tar.zst";
  sha256 = "1ysbxirpfr0yf7pvyps75lnwc897w2a2kcid3nb4j6ilw6n64jmc";
  name = "mingw-w64-clang-x86_64-rhash-1.4.4-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.31.2-3-any.pkg.tar.zst";
  sha256 = "139f91r392c68hsajm0c81690pmzkywb0p4x8ms8ms53ncxnz6gz";
  name = "mingw-w64-clang-x86_64-cmake-3.31.2-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-4.0.0-1-any.pkg.tar.zst";
  sha256 = "0hrhbjgi0g3jqpw8himshqw6vazm5sxhsfmyg386nbrxwnfgl1gb";
  name = "mingw-w64-clang-x86_64-mpdecimal-4.0.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.5.20240831-1-any.pkg.tar.zst";
  sha256 = "1hlfj9g4s767s502sawwbcv4a0xd3ym3ip4jswmhq48wh5050iyb";
  name = "mingw-w64-clang-x86_64-ncurses-6.5.20240831-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
  sha256 = "17ha468qavwin800cc3b7c3xdggwk2gakasfxg7jdx7616d99l0n";
  name = "mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-readline-8.2.013-1-any.pkg.tar.zst";
  sha256 = "0pv1ypqfgm4mimzr0amq9anr1ysqmzrwv6gfk7rrlzhihadknsvr";
  name = "mingw-w64-clang-x86_64-readline-8.2.013-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.47.2-1-any.pkg.tar.zst";
  sha256 = "10pavblv9yjirlm5hix9aikpswhiamry097clba6jcvsajlx4azy";
  name = "mingw-w64-clang-x86_64-sqlite3-3.47.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tcl-8.6.13-1-any.pkg.tar.zst";
  sha256 = "0paaqwk0sfy2zxwlxkmxf2bqq46lyg0sx7cqgzknvazwx8xa2z4x";
  name = "mingw-w64-clang-x86_64-tcl-8.6.13-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tk-8.6.13-1-any.pkg.tar.zst";
  sha256 = "12f6lqx1sglczcnz2ns6sxw9cxwm1klxajqzcrbnfwln1nllz2nd";
  name = "mingw-w64-clang-x86_64-tk-8.6.13-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2024b-1-any.pkg.tar.zst";
  sha256 = "0jihnr1i7vyzczxz60ds1x3gcm3p4ad2pq9d5vvpwjdwrxkvxmkc";
  name = "mingw-w64-clang-x86_64-tzdata-2024b-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.12.7-3-any.pkg.tar.zst";
  sha256 = "1v15j2pzy9wj4n1rjngdi2hf8h0l9z4lri3xb86yvdv1xl2msj6h";
  name = "mingw-w64-clang-x86_64-python-3.12.7-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-19.1.4-2-any.pkg.tar.zst";
  sha256 = "1pn1fbj74rx837s9z8gqs4b0cr7kqi5m1m2mi9ibjpw64m1aqwxv";
  name = "mingw-w64-clang-x86_64-llvm-openmp-19.1.4-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.28-2-any.pkg.tar.zst";
  sha256 = "18p1zhf7h3k3phf3bl483jg3k7y9zq375z6ww75g62158ic9lfyc";
  name = "mingw-w64-clang-x86_64-openblas-0.3.28-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.1.1-2-any.pkg.tar.zst";
  sha256 = "1kiy7ail04ias47xbbhl9vpsz02g0g3f29ncgx5gcks9vgqldp6m";
  name = "mingw-w64-clang-x86_64-python-numpy-2.1.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-75.6.0-1-any.pkg.tar.zst";
  sha256 = "03l04kjmy5p9whaw0h619gdg7yw1gxbz8phifq4pzh3c1wlw7yfd";
  name = "mingw-w64-clang-x86_64-python-setuptools-75.6.0-1-any.pkg.tar.zst";
})
]
