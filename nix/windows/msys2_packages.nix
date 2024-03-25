{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-18.1.2-1-any.pkg.tar.zst";
  sha256 = "0ksz7xz1lbwsmdr9sa1444k0dlfkbd8k11pq7w08ir7r1wjy6fid";
  name = "mingw-w64-clang-x86_64-libunwind-18.1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-18.1.2-1-any.pkg.tar.zst";
  sha256 = "0r8skyjqv4cpkqif0niakx4hdpkscil1zf6mzj34pqna0j5gdnq2";
  name = "mingw-w64-clang-x86_64-libc++-18.1.2-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.6.1-1-any.pkg.tar.zst";
  sha256 = "14p4xxaxjjy6j1ingji82xhai1mc1gls5ali6z40fbb2ylxkaggs";
  name = "mingw-w64-clang-x86_64-xz-5.6.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
  sha256 = "06i9xjsskf4ddb2ph4h31md5c7imj9mzjhd4lc4q44j8dmpc1w5p";
  name = "mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.12.6-1-any.pkg.tar.zst";
  sha256 = "177b3rmsknqq6hf0zqwva71s3avh20ca7vzznp2ls2z5qm8vhhlp";
  name = "mingw-w64-clang-x86_64-libxml2-2.12.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
  sha256 = "07739wmwgxf0d6db4p8w302a6jwcm01aafr1s8jvcl5k1h5a1m2m";
  name = "mingw-w64-clang-x86_64-zstd-1.5.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-18.1.2-1-any.pkg.tar.zst";
  sha256 = "0ibiy01v16naik9pj32ch7a9pkbw4yrn3gyq7p0y6kcc63fkjazy";
  name = "mingw-w64-clang-x86_64-llvm-libs-18.1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-18.1.2-1-any.pkg.tar.zst";
  sha256 = "1hcfz6nb6svmmcqzfrdi96az2x7mzj0cispdv2ssbgn7nkf19pi0";
  name = "mingw-w64-clang-x86_64-llvm-18.1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-18.1.2-1-any.pkg.tar.zst";
  sha256 = "1k17d18g7rmq2ph4kq1mf84vs8133jzf52nkv6syh39ypjga67wa";
  name = "mingw-w64-clang-x86_64-clang-libs-18.1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-18.1.2-1-any.pkg.tar.zst";
  sha256 = "1w2j0vs888haz9shjr1l8dc4j957sk1p0377zzipkbqnzqwjf1z8";
  name = "mingw-w64-clang-x86_64-compiler-rt-18.1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-11.0.0.r655.gdbfdf8025-1-any.pkg.tar.zst";
  sha256 = "18csfwlk2h9pr4411crx1b41qjzn5jgbssm3h109nzwbdizkp62h";
  name = "mingw-w64-clang-x86_64-headers-git-11.0.0.r655.gdbfdf8025-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-11.0.0.r655.gdbfdf8025-1-any.pkg.tar.zst";
  sha256 = "03l1zkrxgxxssp430xcv2gch1d03rbnbk1c0vgiqxigcs8lljh2g";
  name = "mingw-w64-clang-x86_64-crt-git-11.0.0.r655.gdbfdf8025-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-18.1.2-1-any.pkg.tar.zst";
  sha256 = "1ai4gl7ybpk9n10jmbpf3zzfa893m1krj5qhf44ajln0jabdfnbn";
  name = "mingw-w64-clang-x86_64-lld-18.1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-git-11.0.0.r655.gdbfdf8025-1-any.pkg.tar.zst";
  sha256 = "1svhjzwhvl4ldl439jhgfy47g05y2af1cjqvydgijn1dd4g8y8vq";
  name = "mingw-w64-clang-x86_64-libwinpthread-git-11.0.0.r655.gdbfdf8025-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-git-11.0.0.r655.gdbfdf8025-1-any.pkg.tar.zst";
  sha256 = "0jxdhkl256vnr13xf1x3fyjrdf764zg70xcs3gki3rg109f0a6xk";
  name = "mingw-w64-clang-x86_64-winpthreads-git-11.0.0.r655.gdbfdf8025-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-18.1.2-1-any.pkg.tar.zst";
  sha256 = "0ahfic7vdfv96k5v7fdkgk1agk28l833xjn2igrmbvqg96ak0w6n";
  name = "mingw-w64-clang-x86_64-clang-18.1.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.27.0-1-any.pkg.tar.zst";
  sha256 = "06y3sgqv6a0gr3dsbzs36jrj8adklssgjqi2ms5clsyq6ay4f91r";
  name = "mingw-w64-clang-x86_64-c-ares-1.27.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20240203-1-any.pkg.tar.zst";
  sha256 = "1q5nxhsk04gidz66ai5wgd4dr04lfyakkfja9p0r5hrgg4ppqqjg";
  name = "mingw-w64-clang-x86_64-ca-certificates-20240203-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.60.0-1-any.pkg.tar.zst";
  sha256 = "0wxw8266hf4qd2m4zpgb1wvlrnaksmcrs0kh5y9zpf2y5sy8f2bq";
  name = "mingw-w64-clang-x86_64-nghttp2-1.60.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-3.29.0-1-any.pkg.tar.zst";
  sha256 = "0l79lf6zihn0k8hz93qnjnq259y45yq19235g9c444jc2w093si1";
  name = "mingw-w64-clang-x86_64-cmake-3.29.0-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.45.2-1-any.pkg.tar.zst";
  sha256 = "1icvw3f08cgi94p0177i46v72wgpsxw95p6kd0sm2w3vj0qlqbcw";
  name = "mingw-w64-clang-x86_64-sqlite3-3.45.2-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openmp-18.1.2-1-any.pkg.tar.zst";
  sha256 = "1v9wm3ja3a7a7yna2bpqky481qf244wc98kfdl7l03k7rkvvydpl";
  name = "mingw-w64-clang-x86_64-openmp-18.1.2-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-69.1.1-1-any.pkg.tar.zst";
  sha256 = "1mc56anasj0v92nlg84m3pa7dbqgjakxw0b4ibqlrr9cq0xzsg4b";
  name = "mingw-w64-clang-x86_64-python-setuptools-69.1.1-1-any.pkg.tar.zst";
})
]
