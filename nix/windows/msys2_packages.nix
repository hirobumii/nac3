{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.4.4-1-any.pkg.tar.zst";
  sha256 = "0mws1g7w11riczc168x7kzb8nl74iry4bzkb72nspw1vmlblxfy6";
  name = "mingw-w64-clang-x86_64-libffi-3.4.4-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.12.4-1-any.pkg.tar.zst";
  sha256 = "1v72qklxisspyz09y8f00ng4a39h2cjlbrgzfcr8xvlgjg6yz8xc";
  name = "mingw-w64-clang-x86_64-libxml2-2.12.4-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-11.0.0.r551.g86a5e0f41-1-any.pkg.tar.zst";
  sha256 = "10158d7knsj6md8mqnvib7aiyrf6ijqd3302zy7k83nmcghzp15a";
  name = "mingw-w64-clang-x86_64-headers-git-11.0.0.r551.g86a5e0f41-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-11.0.0.r551.g86a5e0f41-1-any.pkg.tar.zst";
  sha256 = "1anhsz8w3hkxbln5apqr2cwr7gcp85lsri69b8zma7hqpdljq3i1";
  name = "mingw-w64-clang-x86_64-crt-git-11.0.0.r551.g86a5e0f41-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-17.0.6-7-any.pkg.tar.zst";
  sha256 = "0v2q0770bavm5nsf57vxb5hf9iz8aip97yy34cd30g6xvx33vz95";
  name = "mingw-w64-clang-x86_64-lld-17.0.6-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-11.0.0.r551.g86a5e0f41-1-any.pkg.tar.zst";
  sha256 = "1bp75rzfrb7d884hxwp704ks51h62wb4jlmw8wlfhl8ffv1i6z0d";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-11.0.0.r551.g86a5e0f41-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-11.0.0.r551.g86a5e0f41-1-any.pkg.tar.zst";
  sha256 = "1vafn4yfmhcq60qx2kyivici4gk4cvmzzg3jhcrsijj46dh9pdj0";
  name = "mingw-w64-clang-x86_64-winpthreads-git-11.0.0.r551.g86a5e0f41-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-17.0.6-7-any.pkg.tar.zst";
  sha256 = "0z6w4iixsl9cswc3mz9saw61dvz1wy1ssfspma2zni6s79igwdbq";
  name = "mingw-w64-clang-x86_64-clang-17.0.6-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.25.0-1-any.pkg.tar.zst";
  sha256 = "1r3ddd96jd97hdxgsqr50ms96nlbkc7b7i7brfyazjsrzl6b1iml";
  name = "mingw-w64-clang-x86_64-c-ares-1.25.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libpsl-0.21.5-1-any.pkg.tar.zst";
  sha256 = "0h8gfifgpk7gvi6f1ca2bll0qi599fjijjvhp0vqxb9yrsgxrjg2";
  name = "mingw-w64-clang-x86_64-libpsl-0.21.5-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.59.0-1-any.pkg.tar.zst";
  sha256 = "1id5nkz8n2d3qxvrvp0zrbycwg1z58qwv5p6msmajx4ra3gkma47";
  name = "mingw-w64-clang-x86_64-nghttp2-1.59.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.5.0-1-any.pkg.tar.zst";
  sha256 = "10xy39wr9f1yaafv5kb2iqjf2xgk005qpzsrc6i6p8p19xhb781s";
  name = "mingw-w64-clang-x86_64-curl-8.5.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.75.0-1-any.pkg.tar.zst";
  sha256 = "08w0k9kpc8a6mjlsyxmlkcnw9abrfx22ckhrq98k5gszhgm5l1ar";
  name = "mingw-w64-clang-x86_64-rust-1.75.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pkgconf-1~2.1.0-1-any.pkg.tar.zst";
  sha256 = "04y364mzx2sj984cdxq187fg73hzkrzvbpsr5d86xfzrag789nh3";
  name = "mingw-w64-clang-x86_64-pkgconf-12.1.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libtre-git-r177.07e66d0-1-any.pkg.tar.zst";
  sha256 = "1j50jkgns4w6ki617jz0f9fpphkhwp3mwkyqagssmvcb79z4pyia";
  name = "mingw-w64-clang-x86_64-libtre-git-r177.07e66d0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.4-2-any.pkg.tar.zst";
  sha256 = "1spwqfx1sv3car1048yx6q928ymagmfp9hijlldlvc9snipbj3jm";
  name = "mingw-w64-clang-x86_64-rhash-1.4.4-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.28.1-1-any.pkg.tar.zst";
  sha256 = "1injc2m95k9xxnh6zfxy43mihdi6w0sb7mfjbivdyzcpcwzhg86w";
  name = "mingw-w64-clang-x86_64-cmake-3.28.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
  sha256 = "0zm9pqcgjk9a8ql6hxcxh477wvvyimndcisd6zrr6y8m5vvklsdi";
  name = "mingw-w64-clang-x86_64-mpdecimal-2.5.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.45.0-1-any.pkg.tar.zst";
  sha256 = "1yb4f5zdbgpi30l2425zppl5yqy7wvdjy99p9mqz04wb5zpghc63";
  name = "mingw-w64-clang-x86_64-sqlite3-3.45.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tk-8.6.12-2-any.pkg.tar.zst";
  sha256 = "0pi74q91vl6vw8vvmmwnvrgai3b1aanp0zhca5qsmv8ljh2wdgzx";
  name = "mingw-w64-clang-x86_64-tk-8.6.12-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2023d-1-any.pkg.tar.zst";
  sha256 = "0g7ijbzm0d86rdkg1lcwaqd9mcn9azv76kxiannvnbhsm8fz8axw";
  name = "mingw-w64-clang-x86_64-tzdata-2023d-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.11.7-1-any.pkg.tar.zst";
  sha256 = "0c7s8y6ndh3q34kgpjgj2yp8ca6ca772p2s877pql3kx428xgwzq";
  name = "mingw-w64-clang-x86_64-python-3.11.7-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-1.26.3-1-any.pkg.tar.zst";
  sha256 = "0i6zpnxj515yfims7q4cgb8z8lip5sdnb5gid7jx0lxs47n2qzv0";
  name = "mingw-w64-clang-x86_64-python-numpy-1.26.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-69.0.3-2-any.pkg.tar.zst";
  sha256 = "0p0xz6rmvr5xijx8x1z23rm8130jjdj0l65i2fz5xh2m0di5ngx9";
  name = "mingw-w64-clang-x86_64-python-setuptools-69.0.3-2-any.pkg.tar.zst";
})
]
