{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-18.1.8-2-any.pkg.tar.zst";
  sha256 = "0f9m76dx40iy794nfks0360gvjhdg6yngb2lyhwp4xd76rn5081m";
  name = "mingw-w64-clang-x86_64-libunwind-18.1.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-18.1.8-2-any.pkg.tar.zst";
  sha256 = "17savj9wys9my2ji7vyba7wwqkvzdjwnkb3k4858wxrjbzbfa6lk";
  name = "mingw-w64-clang-x86_64-libc++-18.1.8-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.6.2-2-any.pkg.tar.zst";
  sha256 = "0phb9hwqksk1rg29yhwlc7si78zav19c2kac0i841pc7mc2n9gzx";
  name = "mingw-w64-clang-x86_64-xz-5.6.2-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
  sha256 = "06i9xjsskf4ddb2ph4h31md5c7imj9mzjhd4lc4q44j8dmpc1w5p";
  name = "mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.12.9-1-any.pkg.tar.zst";
  sha256 = "0cjz2vj9yz6k5xj601cp0yk631rrr0z94ciamwqrvclb0yhakf25";
  name = "mingw-w64-clang-x86_64-libxml2-2.12.9-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.6-2-any.pkg.tar.zst";
  sha256 = "02cp5ci8w50k7xn38mpkwnr8sn898v18wcc07y8f9sfla7vcyfix";
  name = "mingw-w64-clang-x86_64-zstd-1.5.6-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-18.1.8-1-any.pkg.tar.zst";
  sha256 = "0rpbgvvinsqflhd3nhfxk0g0yy8j80zzw5yx6573ak0m78a9fa06";
  name = "mingw-w64-clang-x86_64-llvm-libs-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-18.1.8-1-any.pkg.tar.zst";
  sha256 = "185g5h8q3x3rav9lp2njln58ny2idh2067fd02j3nsbik6glshpf";
  name = "mingw-w64-clang-x86_64-llvm-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-18.1.8-1-any.pkg.tar.zst";
  sha256 = "089hji3yd7wsd03v9mdfgc99l5k1dql8kg7p3hy13vrbgfsabxhc";
  name = "mingw-w64-clang-x86_64-clang-libs-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-18.1.8-1-any.pkg.tar.zst";
  sha256 = "1dwcxnv1k5ljim5ys4h1c3jlrdpi0054z094ynav7if65i8zjj4a";
  name = "mingw-w64-clang-x86_64-compiler-rt-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-12.0.0.r250.gc6bf4bdf6-1-any.pkg.tar.zst";
  sha256 = "0163jzjlvq7inpafy3h48pkwag3ysk6x56xm84yfcz5q52fnfzq5";
  name = "mingw-w64-clang-x86_64-headers-git-12.0.0.r250.gc6bf4bdf6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-12.0.0.r250.gc6bf4bdf6-1-any.pkg.tar.zst";
  sha256 = "00cn1mi29mfys7qy4hvgnjd0smqvnkdn3ibnrr6a3wy1h2vaykgq";
  name = "mingw-w64-clang-x86_64-crt-git-12.0.0.r250.gc6bf4bdf6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-18.1.8-1-any.pkg.tar.zst";
  sha256 = "1vpij5d06m4kjy3qv8bizwlkl21gcv6fv0r2f1j9bclgm6k3144x";
  name = "mingw-w64-clang-x86_64-lld-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r250.gc6bf4bdf6-1-any.pkg.tar.zst";
  sha256 = "1zkzqqd31xpkv817wja3qssjjx891bsdxw07037hv2sk0qr4ffn9";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r250.gc6bf4bdf6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r250.gc6bf4bdf6-1-any.pkg.tar.zst";
  sha256 = "02ynia88ad3l03r08nyldmnajwqkyxcjd191lyamkbj4d6zck323";
  name = "mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r250.gc6bf4bdf6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-18.1.8-1-any.pkg.tar.zst";
  sha256 = "1qny934nv4g75k9gb5sf31v24bgafkg6qw7r35xv3in491w6annq";
  name = "mingw-w64-clang-x86_64-clang-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.33.1-1-any.pkg.tar.zst";
  sha256 = "14r6jjsvfbapbkv2zqp2yglva4vz4srzkgk7f186ri3kcafjspgq";
  name = "mingw-w64-clang-x86_64-c-ares-1.33.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-brotli-1.1.0-2-any.pkg.tar.zst";
  sha256 = "1q01lz9lcyrjmkhv9rddgjazmk7warlcmwhc4qkq9y6h0yfsb71n";
  name = "mingw-w64-clang-x86_64-brotli-1.1.0-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libpsl-0.21.5-2-any.pkg.tar.zst";
  sha256 = "1mpx77q5g8pj45s8wgc52c4ww2r93080p6d559p56f558a3cl317";
  name = "mingw-w64-clang-x86_64-libpsl-0.21.5-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.3.2-1-any.pkg.tar.zst";
  sha256 = "1djgpcz447yvhdy1yq5wh8l5d0821izxklx9afyszbw0pbr7f24y";
  name = "mingw-w64-clang-x86_64-openssl-3.3.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
  sha256 = "0l2m823gm1rvnjmqm5ads17mxz1bhpzai5ixyhnkpzrsjxd1ygy5";
  name = "mingw-w64-clang-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.63.0-1-any.pkg.tar.zst";
  sha256 = "0lfqrlmapsc7ilxjmhr7hxi578vclqlhpqimbvzq0c70c0iwk864";
  name = "mingw-w64-clang-x86_64-nghttp2-1.63.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.5.0-1-any.pkg.tar.zst";
  sha256 = "1ljl9kdasf91bxkqcmbbjchp5g00ahv8jn2zab38899z6j3x43nz";
  name = "mingw-w64-clang-x86_64-nghttp3-1.5.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.9.1-2-any.pkg.tar.zst";
  sha256 = "1zr6kgqp9i4qqrfckh3kfmz4x1cwv4xis9sfqsx7xji88priax64";
  name = "mingw-w64-clang-x86_64-curl-8.9.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.80.1-1-any.pkg.tar.zst";
  sha256 = "1dm4vlrfi9m6xl09zpn0yjr7qcjjr4x738z1rjfwysfnc0awq4x8";
  name = "mingw-w64-clang-x86_64-rust-1.80.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
  sha256 = "0phhwkcqp30dsyj5vr6w99sgm1jfm5rzg0w5x5mv9md4x7lm9lmh";
  name = "mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.6.3-1-any.pkg.tar.zst";
  sha256 = "19xfl1q78q1k8j0lr5aspcf668pmfg01fgib73zq7ff7y5y5fcyi";
  name = "mingw-w64-clang-x86_64-expat-2.6.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-jsoncpp-1.9.5-3-any.pkg.tar.zst";
  sha256 = "1a8mdn4ram9pgqpx5fwxmhcmzc6bh1fq1s4m37xh0d8p6fpncv10";
  name = "mingw-w64-clang-x86_64-jsoncpp-1.9.5-3-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtre-git-r177.07e66d0-2-any.pkg.tar.zst";
  sha256 = "0fc9hxsdks1xy5fv0rcna433hlzf6jhs77hg0hfzkzhn06f9alp4";
  name = "mingw-w64-clang-x86_64-libtre-git-r177.07e66d0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libsystre-1.0.1-5-any.pkg.tar.zst";
  sha256 = "05qsn8fkks4f93jkas43s47axqqgx5m64b45p462si3nlb8cjirq";
  name = "mingw-w64-clang-x86_64-libsystre-1.0.1-5-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.7.4-1-any.pkg.tar.zst";
  sha256 = "1ykw6imllgxv6lsgwxx1miqjr4l1iryqkrj286jcbfrb8ghpzhv5";
  name = "mingw-w64-clang-x86_64-libarchive-3.7.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.48.0-1-any.pkg.tar.zst";
  sha256 = "0kfzanvx7hg7bvy35h2z2vcfxvwn44sikd36mvzhkv6c3c6y84sn";
  name = "mingw-w64-clang-x86_64-libuv-1.48.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.30.3-1-any.pkg.tar.zst";
  sha256 = "0fjwf6xxzli6rcsbzr1razldmm538ibkyf5kw132lpaz5wma9bj8";
  name = "mingw-w64-clang-x86_64-cmake-3.30.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-4.0.0-1-any.pkg.tar.zst";
  sha256 = "0hrhbjgi0g3jqpw8himshqw6vazm5sxhsfmyg386nbrxwnfgl1gb";
  name = "mingw-w64-clang-x86_64-mpdecimal-4.0.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.4.20231217-1-any.pkg.tar.zst";
  sha256 = "00046d52zsr8zjifl7h22jfihhh53h20ipvbqmvf9myssw2fwjza";
  name = "mingw-w64-clang-x86_64-ncurses-6.4.20231217-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tcl-8.6.13-1-any.pkg.tar.zst";
  sha256 = "0paaqwk0sfy2zxwlxkmxf2bqq46lyg0sx7cqgzknvazwx8xa2z4x";
  name = "mingw-w64-clang-x86_64-tcl-8.6.13-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.46.1-1-any.pkg.tar.zst";
  sha256 = "1axplxyjnaz411qzjjqwbj55fbrh4akq3plm2p1sx64jp844xpyq";
  name = "mingw-w64-clang-x86_64-sqlite3-3.46.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tk-8.6.13-1-any.pkg.tar.zst";
  sha256 = "12f6lqx1sglczcnz2ns6sxw9cxwm1klxajqzcrbnfwln1nllz2nd";
  name = "mingw-w64-clang-x86_64-tk-8.6.13-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2024a-1-any.pkg.tar.zst";
  sha256 = "1lsfn3759cyf56zlmfvgy6ihs4iks6zhlnrbfmnq5wml02k936ji";
  name = "mingw-w64-clang-x86_64-tzdata-2024a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python3.12-3.12.1-2-any.pkg.tar.zst";
  sha256 = "0wmd39wl9z237w093a7c6hl5pclca9yvwxn0kiw6i2njk3sjv51a";
  name = "mingw-w64-clang-x86_64-python3.12-3.12.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.11.9-1-any.pkg.tar.zst";
  sha256 = "0ah1idjqxg7jc07a1gz9z766rjjd0f0c6ri4hpcsimsrbj1zjd3c";
  name = "mingw-w64-clang-x86_64-python-3.11.9-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-18.1.8-1-any.pkg.tar.zst";
  sha256 = "0cy2v0l4af24j34mzj5q5nlzcqhackfajlfj1rpf6mb3rbz23qw9";
  name = "mingw-w64-clang-x86_64-llvm-openmp-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.28-1-any.pkg.tar.zst";
  sha256 = "1pskcqc1lg9p8m8rk7bw3mz7mn7vw5fpl7zxa23bhjn02p5b79qq";
  name = "mingw-w64-clang-x86_64-openblas-0.3.28-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.0.1-1-any.pkg.tar.zst";
  sha256 = "0ks6q8v58h4wmr2pzsjl2xm4f63g0psvfm0jwlz24mqfxp8gqfcc";
  name = "mingw-w64-clang-x86_64-python-numpy-2.0.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-74.0.0-1-any.pkg.tar.zst";
  sha256 = "0xc95z5jzzjf5lw35bs4yn5rlwkrkmrh78yi5rranqpz5nn0wsa0";
  name = "mingw-w64-clang-x86_64-python-setuptools-74.0.0-1-any.pkg.tar.zst";
})
]
