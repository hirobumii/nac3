{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.4.5-1-any.pkg.tar.zst";
  sha256 = "13br3j605wn1vmwvfd32c99x247b01dvnkpdbxp0yx7w76f0w8n5";
  name = "mingw-w64-clang-x86_64-libffi-3.4.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-17.0.6-1-any.pkg.tar.zst";
  sha256 = "14qpk7xixmygvli5yx66k1kgc4i31sgqv9zjwvg918bw4771cy1w";
  name = "mingw-w64-clang-x86_64-libunwind-17.0.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-17.0.6-1-any.pkg.tar.zst";
  sha256 = "1m3i8znblmzd3yanwss35wfn4v3373dvgkly1zpzxr87cwprhgw9";
  name = "mingw-w64-clang-x86_64-libc++-17.0.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
  sha256 = "06i9xjsskf4ddb2ph4h31md5c7imj9mzjhd4lc4q44j8dmpc1w5p";
  name = "mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libiconv-1.17-4-any.pkg.tar.zst";
  sha256 = "1g2bkhgf60dywccxw911ydyigf3m25yqfh81m5099swr7mjsmzyf";
  name = "mingw-w64-clang-x86_64-libiconv-1.17-4-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gettext-runtime-0.22.4-6-any.pkg.tar.zst";
  sha256 = "06hanbbcb3zk7b4jlw46kcfxk7xb1fdc0g5wfhm4f2i38wc0c3la";
  name = "mingw-w64-clang-x86_64-gettext-runtime-0.22.4-6-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.4.6-2-any.pkg.tar.zst";
  sha256 = "09fy9g84153ccfwcvb6wp8nz7zl0apbm5qwn1zqjn34287y0b71a";
  name = "mingw-w64-clang-x86_64-xz-5.4.6-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.12.5-1-any.pkg.tar.zst";
  sha256 = "0x3457cbbqadn6nl4pbji4mvc78nngc6r17js5qbzg8ir4rllj5i";
  name = "mingw-w64-clang-x86_64-libxml2-2.12.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
  sha256 = "07739wmwgxf0d6db4p8w302a6jwcm01aafr1s8jvcl5k1h5a1m2m";
  name = "mingw-w64-clang-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-17.0.6-7-any.pkg.tar.zst";
  sha256 = "073dh9s67c982f1k9jlssm0d95ikydnfl3kis70bdjyf874d41v9";
  name = "mingw-w64-clang-x86_64-llvm-libs-17.0.6-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-17.0.6-7-any.pkg.tar.zst";
  sha256 = "17w9dzvfm0w6cxd69vy9mipng9ahhsdwabsrjxgf7dc6fhf7cg01";
  name = "mingw-w64-clang-x86_64-llvm-17.0.6-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-17.0.6-7-any.pkg.tar.zst";
  sha256 = "0fb1jvvvzwnb6f2kjqqy2nagk9wb1brh7q7sx1l1blgpwzb99rgr";
  name = "mingw-w64-clang-x86_64-clang-libs-17.0.6-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-17.0.6-7-any.pkg.tar.zst";
  sha256 = "0lcllzsb4wj761kxijd9n70m50dgq6rp9ks8cqgfdk1b2hyxjhmn";
  name = "mingw-w64-clang-x86_64-compiler-rt-17.0.6-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-11.0.0.r631.ga4c0c1d00-1-any.pkg.tar.zst";
  sha256 = "1f3hlmrhmndqd5f6nb9kjs7z7a2dcnnjwdj6vhnq1zdnb9ng5lsz";
  name = "mingw-w64-clang-x86_64-headers-git-11.0.0.r631.ga4c0c1d00-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-11.0.0.r631.ga4c0c1d00-1-any.pkg.tar.zst";
  sha256 = "1g13b9xr2mw88256m45gy9q6ymgbs9fxc6acz8mvai0bqns3h978";
  name = "mingw-w64-clang-x86_64-crt-git-11.0.0.r631.ga4c0c1d00-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-17.0.6-7-any.pkg.tar.zst";
  sha256 = "0v2q0770bavm5nsf57vxb5hf9iz8aip97yy34cd30g6xvx33vz95";
  name = "mingw-w64-clang-x86_64-lld-17.0.6-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-11.0.0.r631.ga4c0c1d00-1-any.pkg.tar.zst";
  sha256 = "0i3ba2rwpyzai51c66kka2w8hbz7gpcc35pcmki1sskh0m9g33i6";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-11.0.0.r631.ga4c0c1d00-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-11.0.0.r631.ga4c0c1d00-1-any.pkg.tar.zst";
  sha256 = "0m86d2k0axdhspd3j63y8v55q463zghw5b0zq6w4f48cwaj3wvlv";
  name = "mingw-w64-clang-x86_64-winpthreads-git-11.0.0.r631.ga4c0c1d00-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-17.0.6-7-any.pkg.tar.zst";
  sha256 = "0z6w4iixsl9cswc3mz9saw61dvz1wy1ssfspma2zni6s79igwdbq";
  name = "mingw-w64-clang-x86_64-clang-17.0.6-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.26.0-1-any.pkg.tar.zst";
  sha256 = "18rzy1rsb25gs4rj258pa35fnlb6ri1bx54s3f52585anna75j19";
  name = "mingw-w64-clang-x86_64-c-ares-1.26.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-p11-kit-0.25.3-2-any.pkg.tar.zst";
  sha256 = "1jrwkc4lvw5hm5rqmi5gqh7mfkbqfa5gi81zjij0krnl0gaxw3c8";
  name = "mingw-w64-clang-x86_64-p11-kit-0.25.3-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20230311-1-any.pkg.tar.zst";
  sha256 = "00hdl239695xi5bgld7a1ssp6kapkb9az02dpx80vmz7mqg6wwxx";
  name = "mingw-w64-clang-x86_64-ca-certificates-20230311-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.2.1-1-any.pkg.tar.zst";
  sha256 = "0ix2r4ll09m2z5vz2k94gmwfs0pp3ipvjdimwzx7v6xhcs2l25lz";
  name = "mingw-w64-clang-x86_64-openssl-3.2.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
  sha256 = "0l2m823gm1rvnjmqm5ads17mxz1bhpzai5ixyhnkpzrsjxd1ygy5";
  name = "mingw-w64-clang-x86_64-libssh2-1.11.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.59.0-1-any.pkg.tar.zst";
  sha256 = "1id5nkz8n2d3qxvrvp0zrbycwg1z58qwv5p6msmajx4ra3gkma47";
  name = "mingw-w64-clang-x86_64-nghttp2-1.59.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.6.0-1-any.pkg.tar.zst";
  sha256 = "1racc7cyzj22kink9w8m8jv73ji5hfg6r6d1ka9dqmvcbx04r8p0";
  name = "mingw-w64-clang-x86_64-curl-8.6.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.76.0-1-any.pkg.tar.zst";
  sha256 = "0ny3bvwvn5wmqrxzhdfw34akr0kj0m7rg9lg3w5yibqz2mkqhk11";
  name = "mingw-w64-clang-x86_64-rust-1.76.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pkgconf-1~2.1.1-1-any.pkg.tar.zst";
  sha256 = "00kxqg9ds4q74lxrzjh8z0858smqbi1j9r06s0zjadsql0ln98cq";
  name = "mingw-w64-clang-x86_64-pkgconf-12.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.6.0-1-any.pkg.tar.zst";
  sha256 = "1zdrv2k04qpzqn90v5g77mcqr5fcfqm83da3i75whwkjydp5szfj";
  name = "mingw-w64-clang-x86_64-expat-2.6.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.7.2-1-any.pkg.tar.zst";
  sha256 = "1p84yh6yzkdpmr02vyvgz16x5gycckah25jkdc2py09l7iw96bmw";
  name = "mingw-w64-clang-x86_64-libarchive-3.7.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.48.0-1-any.pkg.tar.zst";
  sha256 = "0kfzanvx7hg7bvy35h2z2vcfxvwn44sikd36mvzhkv6c3c6y84sn";
  name = "mingw-w64-clang-x86_64-libuv-1.48.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ninja-1.11.1-3-any.pkg.tar.zst";
  sha256 = "13wjfmyfr952n3ydpldjlwx1nla5xpyvr96ng8pfbyw4z900v5ms";
  name = "mingw-w64-clang-x86_64-ninja-1.11.1-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.4-3-any.pkg.tar.zst";
  sha256 = "1ysbxirpfr0yf7pvyps75lnwc897w2a2kcid3nb4j6ilw6n64jmc";
  name = "mingw-w64-clang-x86_64-rhash-1.4.4-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.28.3-2-any.pkg.tar.zst";
  sha256 = "1brv240jiw0sas8pvapyk9s5c3dhynq1cxkr9dcjr5b2rigmq3i3";
  name = "mingw-w64-clang-x86_64-cmake-3.28.3-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.45.1-1-any.pkg.tar.zst";
  sha256 = "04mrbn2b1ylr0vfcsmdbr22xp13y8dvyxhzc6xwnrd9yld3ylfpd";
  name = "mingw-w64-clang-x86_64-sqlite3-3.45.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tk-8.6.12-2-any.pkg.tar.zst";
  sha256 = "0pi74q91vl6vw8vvmmwnvrgai3b1aanp0zhca5qsmv8ljh2wdgzx";
  name = "mingw-w64-clang-x86_64-tk-8.6.12-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2024a-1-any.pkg.tar.zst";
  sha256 = "1lsfn3759cyf56zlmfvgy6ihs4iks6zhlnrbfmnq5wml02k936ji";
  name = "mingw-w64-clang-x86_64-tzdata-2024a-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.11.8-1-any.pkg.tar.zst";
  sha256 = "0djpf4k8s25nys6nrm2x2v134lcgzhhbjs37ihkg0b3sxmmc3b0p";
  name = "mingw-w64-clang-x86_64-python-3.11.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openmp-17.0.6-1-any.pkg.tar.zst";
  sha256 = "0v6ha1c571glq8ghgv4dwwd6v02bk5livmh4pgyyy10awd8zsy20";
  name = "mingw-w64-clang-x86_64-openmp-17.0.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.26-1-any.pkg.tar.zst";
  sha256 = "0kdr72y5lc9dl9s1bjrw8g21qmv2iwd1xvn1r21170i277wsmqiv";
  name = "mingw-w64-clang-x86_64-openblas-0.3.26-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-1.26.4-1-any.pkg.tar.zst";
  sha256 = "00h0ap954cjwlsc3p01fjwy7s3nlzs90v0kmnrzxm0rljmvn4jkf";
  name = "mingw-w64-clang-x86_64-python-numpy-1.26.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-69.1.0-1-any.pkg.tar.zst";
  sha256 = "16s4v18yi0xm10dkk7k5g9nk3ssgq1lplgci2fgq447x1x1cz0sy";
  name = "mingw-w64-clang-x86_64-python-setuptools-69.1.0-1-any.pkg.tar.zst";
})
]
