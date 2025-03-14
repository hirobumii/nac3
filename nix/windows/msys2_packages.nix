{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-20.1.0-1-any.pkg.tar.zst";
  sha256 = "1b70xj60kbnydmgvncav5hyg0n2i16i4hvkkqwnv8d5fkdc74sv2";
  name = "mingw-w64-clang-x86_64-libunwind-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-20.1.0-1-any.pkg.tar.zst";
  sha256 = "1mrmwjx769zqq1qd7smf4ma7ggmqx46f47vypwmh3vzg266h1hzl";
  name = "mingw-w64-clang-x86_64-libc++-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.4.7-1-any.pkg.tar.zst";
  sha256 = "0yi36mq6gzf4bdmx19fh9wzq7592q2c0zq0zj6rc0zn0bglcnm0r";
  name = "mingw-w64-clang-x86_64-libffi-3.4.7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libiconv-1.18-1-any.pkg.tar.zst";
  sha256 = "0vn5xgx9jjg66f8r9ylm9220qdbjdkffykfl6nwj14zv9y7xh4nj";
  name = "mingw-w64-clang-x86_64-libiconv-1.18-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gettext-runtime-0.24-1-any.pkg.tar.zst";
  sha256 = "1d9mhn6sh3baj8zibkravgycqxr0lfargjwg3l7rc1k464dl7a22";
  name = "mingw-w64-clang-x86_64-gettext-runtime-0.24-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.6.4-1-any.pkg.tar.zst";
  sha256 = "121srzvczq16aq0ki6yyym17lnjp6i4hka5wmydjvgrr7nbgik64";
  name = "mingw-w64-clang-x86_64-xz-5.6.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
  sha256 = "06i9xjsskf4ddb2ph4h31md5c7imj9mzjhd4lc4q44j8dmpc1w5p";
  name = "mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.12.10-1-any.pkg.tar.zst";
  sha256 = "1dch932lvsjfwp13cjm5d8sv84ql22c3gfjdrd4vayr8cpykfw4y";
  name = "mingw-w64-clang-x86_64-libxml2-2.12.10-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
  sha256 = "1hrx54k2s3dcs8fhwdwms5amr4gjid1d20b2b4302xyjg9yyvpxl";
  name = "mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-20.1.0-1-any.pkg.tar.zst";
  sha256 = "1amply0fpympi028jdblzfp2cv87l569gi0d2gwhbdd5yq8ga8rp";
  name = "mingw-w64-clang-x86_64-llvm-libs-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-20.1.0-1-any.pkg.tar.zst";
  sha256 = "0p53l7jawgflpfrqdhgc8q9frzrm8liyyxbkxzzs5h79b835yvxp";
  name = "mingw-w64-clang-x86_64-llvm-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-20.1.0-1-any.pkg.tar.zst";
  sha256 = "16c1cn7blfig75k8fd57ssrbnrl2ww20sb42n1c6sfxd1bivmz7b";
  name = "mingw-w64-clang-x86_64-clang-libs-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-20.1.0-1-any.pkg.tar.zst";
  sha256 = "1lg836llv1qlaiqsw0c5amf399qfqaqxm1bgjk4ac02a397dm7k0";
  name = "mingw-w64-clang-x86_64-compiler-rt-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-12.0.0.r576.g49111ba98-1-any.pkg.tar.zst";
  sha256 = "00j7bbj7w9irri90bsxdk5v15kpjk3af3w2xa9i83rrp9ipgclm9";
  name = "mingw-w64-clang-x86_64-headers-git-12.0.0.r576.g49111ba98-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-12.0.0.r576.g49111ba98-1-any.pkg.tar.zst";
  sha256 = "0yrr4yabjkd9nkg78xcmav5gq0xjs8hrwxm22481lj5cvy1r7j70";
  name = "mingw-w64-clang-x86_64-crt-git-12.0.0.r576.g49111ba98-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-20.1.0-1-any.pkg.tar.zst";
  sha256 = "1wbj7m3310y2vja4yxac8akhh60csa5vdwj7dgvzsf511sr05kiz";
  name = "mingw-w64-clang-x86_64-lld-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r576.g49111ba98-1-any.pkg.tar.zst";
  sha256 = "1kgchzv0vihwahrq6jgs1bg14wx0shqp15wrkn45m124m26kb4kd";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r576.g49111ba98-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r576.g49111ba98-1-any.pkg.tar.zst";
  sha256 = "13kj1sd5i4qdqqfqxlj06q52rj6lwg18yi6y7xa0a2yng4i50qrq";
  name = "mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r576.g49111ba98-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-20.1.0-1-any.pkg.tar.zst";
  sha256 = "16bwq1bmk7q4dwwdhcv82y7c23xwgphlzq77siyax4rkp63dk7cd";
  name = "mingw-w64-clang-x86_64-clang-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.85.0-1-any.pkg.tar.zst";
  sha256 = "0wwpmyc0sj3qzafg4c68nx23lw7y80g6awz3sj24zf5cjmwrrv53";
  name = "mingw-w64-clang-x86_64-rust-1.85.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtasn1-4.20.0-1-any.pkg.tar.zst";
  sha256 = "0hv0xayhzhpwp8bdcs2r4xdvimk6266h68ki8abnii0pqiwfi86r";
  name = "mingw-w64-clang-x86_64-libtasn1-4.20.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.4.1-1-any.pkg.tar.zst";
  sha256 = "0c5n7pib853xpgqxs0lb9gdaz4lbia94nf2jps1j7rispspclwwk";
  name = "mingw-w64-clang-x86_64-openssl-3.4.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.1-1-any.pkg.tar.zst";
  sha256 = "01l23cn5brficjzba7ldscqkdvk4rdcvvdyybd90qr2hqzligmhn";
  name = "mingw-w64-clang-x86_64-libssh2-1.11.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.65.0-1-any.pkg.tar.zst";
  sha256 = "14l9l6x5wijyz5r06n2almkk0pq413909in40pdbmnwy2cnd531h";
  name = "mingw-w64-clang-x86_64-nghttp2-1.65.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.8.0-1-any.pkg.tar.zst";
  sha256 = "0xwhnfhhh7xyf2zhpxzi441b1c6vil321r0rsxyr8bqyq4l0marz";
  name = "mingw-w64-clang-x86_64-nghttp3-1.8.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.12.1-1-any.pkg.tar.zst";
  sha256 = "05hq63qibshgar42vi6dsrcph3m1i0khnr2xdkvdqjjys7badrvp";
  name = "mingw-w64-clang-x86_64-curl-8.12.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.50.0-2-any.pkg.tar.zst";
  sha256 = "0jwwm5hbcq7652fr0187jrl2iwiakjchzzxxzdgpm33an5dinsd0";
  name = "mingw-w64-clang-x86_64-libuv-1.50.0-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.31.6-1-any.pkg.tar.zst";
  sha256 = "111fs7i69f6xwjnsl4v3ca38wig2047ri8j0s8mmsc50dgzcdcc2";
  name = "mingw-w64-clang-x86_64-cmake-3.31.6-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2025a-1-any.pkg.tar.zst";
  sha256 = "1q09i8lj96m1kwkxh0qs96ssam8a6vpiylrlf4i55zrazrj9kvk1";
  name = "mingw-w64-clang-x86_64-tzdata-2025a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.12.9-3-any.pkg.tar.zst";
  sha256 = "054v2fw5rbj9n264alpav50573npa706lsmlxilcyc4xxc87k9v9";
  name = "mingw-w64-clang-x86_64-python-3.12.9-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-20.1.0-1-any.pkg.tar.zst";
  sha256 = "1bhz2hpyzklf7pfdcbmmqgqglm912w444ic2x7nvg862b8rxax8z";
  name = "mingw-w64-clang-x86_64-llvm-openmp-20.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.29-1-any.pkg.tar.zst";
  sha256 = "006f2s12jmk35rppkp20rlm7k4kknsnh5h4krqs2ry2rd6qqkk9h";
  name = "mingw-w64-clang-x86_64-openblas-0.3.29-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.2.3-1-any.pkg.tar.zst";
  sha256 = "0jzx828gb3d1pw7w4asv4jhp4yi802qdllkvb1c58msmn43pp76b";
  name = "mingw-w64-clang-x86_64-python-numpy-2.2.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-75.8.0-1-any.pkg.tar.zst";
  sha256 = "12ivpaj967y4bi8396q3fpii4fy5aakidxpv16rkyg1b831k0h93";
  name = "mingw-w64-clang-x86_64-python-setuptools-75.8.0-1-any.pkg.tar.zst";
})
]
