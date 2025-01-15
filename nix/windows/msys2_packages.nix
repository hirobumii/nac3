{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-19.1.6-1-any.pkg.tar.zst";
  sha256 = "1gv6hbqvfgjzirpljql1shlchldmf5ww3rfsspg90pq1frnwavjl";
  name = "mingw-w64-clang-x86_64-libunwind-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-19.1.6-1-any.pkg.tar.zst";
  sha256 = "1wbkvrx14ahc04cgkydvlxwmsl8jfnqwhy9sy4kn4wkdzmlcp1ax";
  name = "mingw-w64-clang-x86_64-libc++-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.4.6-1-any.pkg.tar.zst";
  sha256 = "1q6gms980985bp087rnnpvz2fwfakgm5266izfk3b1mbp620s1yv";
  name = "mingw-w64-clang-x86_64-libffi-3.4.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libiconv-1.18-1-any.pkg.tar.zst";
  sha256 = "0vn5xgx9jjg66f8r9ylm9220qdbjdkffykfl6nwj14zv9y7xh4nj";
  name = "mingw-w64-clang-x86_64-libiconv-1.18-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gettext-runtime-0.23.1-1-any.pkg.tar.zst";
  sha256 = "0wbp5pmrr0rk4mx7d1frvqlk4a061zw31zscs57srmvl0wv3pi2a";
  name = "mingw-w64-clang-x86_64-gettext-runtime-0.23.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-19.1.6-1-any.pkg.tar.zst";
  sha256 = "0fpsnfyf0bg39a4ygzga06sr4wv4jp1jnc8lk6sr3z0nim0nlhjn";
  name = "mingw-w64-clang-x86_64-llvm-libs-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-19.1.6-1-any.pkg.tar.zst";
  sha256 = "0whqs9nvfmgxj3c83px6dipcdw9zi858kgd8130201fy1mbnafp1";
  name = "mingw-w64-clang-x86_64-llvm-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-19.1.6-1-any.pkg.tar.zst";
  sha256 = "0rmzri7h043i73jy3c2jcrg3hy40dr5s9n96kmxgaghfhvlpilps";
  name = "mingw-w64-clang-x86_64-clang-libs-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-19.1.6-1-any.pkg.tar.zst";
  sha256 = "04cqlh35asvlh06nmhwnx9h0yrqk8zxd9lpzxmm1xh64kvm9maxn";
  name = "mingw-w64-clang-x86_64-compiler-rt-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-12.0.0.r473.gce0d0bfb7-1-any.pkg.tar.zst";
  sha256 = "05zsqgq8zwdcfacyqdxdjcf80447bgnrz71xv5cds0y135yziy7l";
  name = "mingw-w64-clang-x86_64-headers-git-12.0.0.r473.gce0d0bfb7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-12.0.0.r473.gce0d0bfb7-1-any.pkg.tar.zst";
  sha256 = "12fkxpk7rwy36snvvc7sdivx81pd4ckzh5ilyh7gl6ly4qayppp6";
  name = "mingw-w64-clang-x86_64-crt-git-12.0.0.r473.gce0d0bfb7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-19.1.6-1-any.pkg.tar.zst";
  sha256 = "102bbv5acq1fvrfn8bp1x3503cb8hvcxmlpr86qsba4vm11l0wrw";
  name = "mingw-w64-clang-x86_64-lld-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r473.gce0d0bfb7-1-any.pkg.tar.zst";
  sha256 = "1sris0qczxk5px9xy85976hbmqrpg49ns7yyzd9p455ckf740cid";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r473.gce0d0bfb7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r473.gce0d0bfb7-1-any.pkg.tar.zst";
  sha256 = "1r0m5xpsxdl00a2daj4p0wgl6037700pvw6p6zl91h1dr092r6pa";
  name = "mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r473.gce0d0bfb7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-19.1.6-1-any.pkg.tar.zst";
  sha256 = "0j4a642fpnvqs79chhinc8r5q53q1wllmc1bzb01a4y7w9rqg4hw";
  name = "mingw-w64-clang-x86_64-clang-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.84.0-1-any.pkg.tar.zst";
  sha256 = "0nrz9788grl50nkbhxswry143rrwpdnc6pk6f0k30kcp19qq6y2d";
  name = "mingw-w64-clang-x86_64-rust-1.84.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
  sha256 = "0phhwkcqp30dsyj5vr6w99sgm1jfm5rzg0w5x5mv9md4x7lm9lmh";
  name = "mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.34.4-1-any.pkg.tar.zst";
  sha256 = "1dppwwx3wrn0lzrlk2q7bpsainbidrpw1ndp1aasyv42xhxl1sn1";
  name = "mingw-w64-clang-x86_64-c-ares-1.34.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-brotli-1.1.0-4-any.pkg.tar.zst";
  sha256 = "0hx9gjzibacfx3fzk11n2vzz2pmnb956babh2ig8avx3hk7vlqrg";
  name = "mingw-w64-clang-x86_64-brotli-1.1.0-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunistring-1.3-1-any.pkg.tar.zst";
  sha256 = "1zg58qbfybyqzcj0dalb13l48f9jsras318h02rka65r7wi0pdcg";
  name = "mingw-w64-clang-x86_64-libunistring-1.3-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20241223-1-any.pkg.tar.zst";
  sha256 = "0c36lg63imzw8i6j1ard42v5wgzpc83phzk8lvifvm0djndq2bbj";
  name = "mingw-w64-clang-x86_64-ca-certificates-20241223-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.7.0-1-any.pkg.tar.zst";
  sha256 = "0kd2f7yh90815kyldxvdy8c6jyxyw0wv4f7k3shwp98w874m0mxd";
  name = "mingw-w64-clang-x86_64-nghttp3-1.7.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.5-1-any.pkg.tar.zst";
  sha256 = "0gdn1351knjwgsqgyaa3l55qs135k7dn6mlf04vzjxlc1895wx5z";
  name = "mingw-w64-clang-x86_64-rhash-1.4.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.31.4-1-any.pkg.tar.zst";
  sha256 = "1xjjwgkqf2j97pcx0yd6j0lgmzgbgqjjf0s7j29mc03g89fhdhw0";
  name = "mingw-w64-clang-x86_64-cmake-3.31.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-4.0.0-1-any.pkg.tar.zst";
  sha256 = "0hrhbjgi0g3jqpw8himshqw6vazm5sxhsfmyg386nbrxwnfgl1gb";
  name = "mingw-w64-clang-x86_64-mpdecimal-4.0.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.5.20241228-3-any.pkg.tar.zst";
  sha256 = "0f98pzrwsxil90n55hz2ym2x2rzrrjrmnj8i2203n189qbxbg2c9";
  name = "mingw-w64-clang-x86_64-ncurses-6.5.20241228-3-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.12.8-2-any.pkg.tar.zst";
  sha256 = "0lksgrmylvpr7yyjcc1szm30pnag7ixrj7vhdql1ryi4k9309v8s";
  name = "mingw-w64-clang-x86_64-python-3.12.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-19.1.6-1-any.pkg.tar.zst";
  sha256 = "0d3mm26hnw716n0ppzqhydxcgm4im081hiiy6l4zp267ad3kfg93";
  name = "mingw-w64-clang-x86_64-llvm-openmp-19.1.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.29-1-any.pkg.tar.zst";
  sha256 = "006f2s12jmk35rppkp20rlm7k4kknsnh5h4krqs2ry2rd6qqkk9h";
  name = "mingw-w64-clang-x86_64-openblas-0.3.29-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.2.1-1-any.pkg.tar.zst";
  sha256 = "0sgkhax9cwmkkrfrir45l91h6pgg339gaw6147gsayf8h8ag4brg";
  name = "mingw-w64-clang-x86_64-python-numpy-2.2.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-75.8.0-1-any.pkg.tar.zst";
  sha256 = "12ivpaj967y4bi8396q3fpii4fy5aakidxpv16rkyg1b831k0h93";
  name = "mingw-w64-clang-x86_64-python-setuptools-75.8.0-1-any.pkg.tar.zst";
})
]
