{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-18.1.8-1-any.pkg.tar.zst";
  sha256 = "1v8zkfcbf1ga2ndpd1j0dwv5s1rassxs2b5pjhcsmqwjcvczba1m";
  name = "mingw-w64-clang-x86_64-libunwind-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-18.1.8-1-any.pkg.tar.zst";
  sha256 = "0mfd8wrmgx12j5gf354j7pk1l3lg9ykxvq75xdk3jipsr6hbn846";
  name = "mingw-w64-clang-x86_64-libc++-18.1.8-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.12.8-1-any.pkg.tar.zst";
  sha256 = "1imipb0dz4w6x4n9arn22imyzzcwdlf2cqxvn7irqq7w9by6fy0b";
  name = "mingw-w64-clang-x86_64-libxml2-2.12.8-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-12.0.0.r81.g90abf784a-1-any.pkg.tar.zst";
  sha256 = "1h3cdcajz29iq7vja908kkijz1vb9xn0f7w1lw1ima0q0zhinv4q";
  name = "mingw-w64-clang-x86_64-headers-git-12.0.0.r81.g90abf784a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-12.0.0.r81.g90abf784a-1-any.pkg.tar.zst";
  sha256 = "15kamyi3b0j6f5zxin4i2jgzjc7lzvwl4z5cz3dx0i8hg91aq0n7";
  name = "mingw-w64-clang-x86_64-crt-git-12.0.0.r81.g90abf784a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-18.1.8-1-any.pkg.tar.zst";
  sha256 = "1vpij5d06m4kjy3qv8bizwlkl21gcv6fv0r2f1j9bclgm6k3144x";
  name = "mingw-w64-clang-x86_64-lld-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r81.g90abf784a-1-any.pkg.tar.zst";
  sha256 = "0qdvgs1rmjjhn9klf9kpw7l0ydz36rr5fasn4q9gpby2lgl11bkb";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-12.0.0.r81.g90abf784a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r81.g90abf784a-1-any.pkg.tar.zst";
  sha256 = "0rh2mn078cifcmr4as4k57jxjln5lbnsmpx47h9d0s5d2i8sf2rc";
  name = "mingw-w64-clang-x86_64-winpthreads-git-12.0.0.r81.g90abf784a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-18.1.8-1-any.pkg.tar.zst";
  sha256 = "1qny934nv4g75k9gb5sf31v24bgafkg6qw7r35xv3in491w6annq";
  name = "mingw-w64-clang-x86_64-clang-18.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.29.0-1-any.pkg.tar.zst";
  sha256 = "01xg1h1a8kda0kq2921w25ybvm1ms7lfdzday0hv93f3myq7briq";
  name = "mingw-w64-clang-x86_64-c-ares-1.29.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-brotli-1.1.0-1-any.pkg.tar.zst";
  sha256 = "113mha41q53cx0hw13cq1xdf7zbsd58sh8cl1cd7xzg1q69n60w2";
  name = "mingw-w64-clang-x86_64-brotli-1.1.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.3.1-1-any.pkg.tar.zst";
  sha256 = "0ywhwm4kw3qjzv0872qwabnsq2rzbmqjb9m69q3fykjl0m9gigsa";
  name = "mingw-w64-clang-x86_64-openssl-3.3.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
  sha256 = "0l2m823gm1rvnjmqm5ads17mxz1bhpzai5ixyhnkpzrsjxd1ygy5";
  name = "mingw-w64-clang-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.61.0-2-any.pkg.tar.zst";
  sha256 = "07bkk98126gy4k6lb9rrqqnzjfz9j2rsr5dzr2djmzdkw0h4dr95";
  name = "mingw-w64-clang-x86_64-nghttp2-1.61.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.4.0-1-any.pkg.tar.zst";
  sha256 = "007w2252nzn274j4wjc1vf56xyzzh5vg3blj1hil7mlmffgvc923";
  name = "mingw-w64-clang-x86_64-nghttp3-1.4.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.8.0-10-any.pkg.tar.zst";
  sha256 = "024z5b1achkf448gxqy1i3gcw371x54kfl6igv08b5wb3rrw35a4";
  name = "mingw-w64-clang-x86_64-curl-8.8.0-10-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.79.0-1-any.pkg.tar.zst";
  sha256 = "0i7s88hj8m4920xifkj7i4b4sq8cqq7p5cypp3jqx3dc44pwm19a";
  name = "mingw-w64-clang-x86_64-rust-1.79.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pkgconf-1~2.2.0-1-any.pkg.tar.zst";
  sha256 = "1y44ijg3y8p80f1yn9972nshrnyrd06a9sh984ajhxg8bi8s5xyl";
  name = "mingw-w64-clang-x86_64-pkgconf-12.2.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.6.2-1-any.pkg.tar.zst";
  sha256 = "0kj1vzjh3qh7d2g47avlgk7a6j4nc62111hy1m63jwq0alc01k38";
  name = "mingw-w64-clang-x86_64-expat-2.6.2-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lz4-1.9.4-1-any.pkg.tar.zst";
  sha256 = "0nn7cy25j53q5ckkx4n4f77w00xdwwf5wjswm374shvvs58nlln0";
  name = "mingw-w64-clang-x86_64-lz4-1.9.4-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.4-3-any.pkg.tar.zst";
  sha256 = "1ysbxirpfr0yf7pvyps75lnwc897w2a2kcid3nb4j6ilw6n64jmc";
  name = "mingw-w64-clang-x86_64-rhash-1.4.4-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.30.0-1-any.pkg.tar.zst";
  sha256 = "07b7132hwhiqrf0l2lgw3g4zw9i2lln3kqc9kg2qijvkapbkmwqb";
  name = "mingw-w64-clang-x86_64-cmake-3.30.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-readline-8.2.010-1-any.pkg.tar.zst";
  sha256 = "1s47pd5iz8y3hspsxn4pnp0v3m05ccia40v5nfvx0rmwgvcaz82v";
  name = "mingw-w64-clang-x86_64-readline-8.2.010-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tcl-8.6.13-1-any.pkg.tar.zst";
  sha256 = "0paaqwk0sfy2zxwlxkmxf2bqq46lyg0sx7cqgzknvazwx8xa2z4x";
  name = "mingw-w64-clang-x86_64-tcl-8.6.13-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.46.0-1-any.pkg.tar.zst";
  sha256 = "0q676i2z5nr4c71jnd4z5qz9xa1xryl0cpi84w74yvd0p4qiz7y2";
  name = "mingw-w64-clang-x86_64-sqlite3-3.46.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.27-1-any.pkg.tar.zst";
  sha256 = "06ygz1wa488wqvmxbn74b0fyan4wf3lb6kbwfampgikd1gijww2k";
  name = "mingw-w64-clang-x86_64-openblas-0.3.27-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-1.26.4-1-any.pkg.tar.zst";
  sha256 = "00h0ap954cjwlsc3p01fjwy7s3nlzs90v0kmnrzxm0rljmvn4jkf";
  name = "mingw-w64-clang-x86_64-python-numpy-1.26.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-70.2.0-1-any.pkg.tar.zst";
  sha256 = "1q4r9bg2hn3jmshvq81xm5zvy9wn35yf0z2ayksrkwph1zzdkvkm";
  name = "mingw-w64-clang-x86_64-python-setuptools-70.2.0-1-any.pkg.tar.zst";
})
]
