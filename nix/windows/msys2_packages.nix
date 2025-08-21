{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-20.1.8-1-any.pkg.tar.zst";
  sha256 = "01mxlqgf9dfhp30sqhi4fbsgmww44xh0qjmyl8drf3gskqyxa2ix";
  name = "mingw-w64-clang-x86_64-libunwind-20.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-20.1.8-1-any.pkg.tar.zst";
  sha256 = "0am1zbfwyywlpbgj7q4h1w8willqqmrg27mm14h6ldrywpnyksvl";
  name = "mingw-w64-clang-x86_64-libc++-20.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.5.1-1-any.pkg.tar.zst";
  sha256 = "1vyc9kyqzwx6p9bxm1ybf38j2072ni25wsi6pyzlqs4y01kqy7ph";
  name = "mingw-w64-clang-x86_64-libffi-3.5.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libiconv-1.18-1-any.pkg.tar.zst";
  sha256 = "0vn5xgx9jjg66f8r9ylm9220qdbjdkffykfl6nwj14zv9y7xh4nj";
  name = "mingw-w64-clang-x86_64-libiconv-1.18-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gettext-runtime-0.26-1-any.pkg.tar.zst";
  sha256 = "165hsrsp4n70klhr1sy1yk966wna173rmha97dzxr9261dbl543g";
  name = "mingw-w64-clang-x86_64-gettext-runtime-0.26-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-xz-5.8.1-2-any.pkg.tar.zst";
  sha256 = "1mms5mk0qqp5hlwxkcrcjr8dv77jvnay6527z4886n1a99mlsniv";
  name = "mingw-w64-clang-x86_64-xz-5.8.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
  sha256 = "06i9xjsskf4ddb2ph4h31md5c7imj9mzjhd4lc4q44j8dmpc1w5p";
  name = "mingw-w64-clang-x86_64-zlib-1.3.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.14.5-2-any.pkg.tar.zst";
  sha256 = "0pdx9i0bswjcx5mfx3syzzw6rx5iwd4wr4w743zklpinyy43yc8q";
  name = "mingw-w64-clang-x86_64-libxml2-2.14.5-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
  sha256 = "1hrx54k2s3dcs8fhwdwms5amr4gjid1d20b2b4302xyjg9yyvpxl";
  name = "mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-20.1.8-2-any.pkg.tar.zst";
  sha256 = "0y3hkfc7y2gmglwszm9l3n1hgc27bcdq39cy5id4l5r62d6r4s5y";
  name = "mingw-w64-clang-x86_64-llvm-libs-20.1.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-20.1.8-2-any.pkg.tar.zst";
  sha256 = "09m1781imh789am14vh427md96304xy5yb6ad7f39g0mjj5z1idy";
  name = "mingw-w64-clang-x86_64-clang-libs-20.1.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-20.1.8-2-any.pkg.tar.zst";
  sha256 = "0sgnbcngspqy0vaavf458b0fj7yglpfv7d6s3sfxb19zz93h8c4g";
  name = "mingw-w64-clang-x86_64-compiler-rt-20.1.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-tools-20.1.8-2-any.pkg.tar.zst";
  sha256 = "0c1ybxsv17a7lbflwp4vf218alpy31hp2bwdsdk06r31ji60z09x";
  name = "mingw-w64-clang-x86_64-llvm-tools-20.1.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-13.0.0.r113.g3692f3ae0-1-any.pkg.tar.zst";
  sha256 = "0psxracbgccy9s69wx8kpvh1xg4h90hhfh99j5m28z2j2wrl848x";
  name = "mingw-w64-clang-x86_64-headers-git-13.0.0.r113.g3692f3ae0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-13.0.0.r113.g3692f3ae0-1-any.pkg.tar.zst";
  sha256 = "0qwpi48sx32iszkizcr15x1rpi9wyp0y10sa8rv5jnj435z1l21y";
  name = "mingw-w64-clang-x86_64-crt-git-13.0.0.r113.g3692f3ae0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-20.1.8-2-any.pkg.tar.zst";
  sha256 = "132xdqkgspn1lfbixrqzyxl66bvngzcl5h4lzknrxbzh9xc6l10l";
  name = "mingw-w64-clang-x86_64-lld-20.1.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-13.0.0.r113.g3692f3ae0-1-any.pkg.tar.zst";
  sha256 = "1slzpw80yr15dby437bbpy8bjiky11h4jf283nhpx9bpv7f96ra7";
  name = "mingw-w64-clang-x86_64-libwinpthread-13.0.0.r113.g3692f3ae0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-13.0.0.r113.g3692f3ae0-1-any.pkg.tar.zst";
  sha256 = "13aimb29146n98lxrxvnlm5hmqhjl23jaql75gyiprkx35jns5nh";
  name = "mingw-w64-clang-x86_64-winpthreads-13.0.0.r113.g3692f3ae0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-20.1.8-2-any.pkg.tar.zst";
  sha256 = "014z7sclpn6j77117am6srvgwjgxmjx2bg8byq33pf0bq76cvv8v";
  name = "mingw-w64-clang-x86_64-clang-20.1.8-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-http-parser-2.9.4-2-any.pkg.tar.zst";
  sha256 = "0bmnpq7cqihspyma4xxg3rsz4z8wqc294pi7wfy2vxn26m82rfy5";
  name = "mingw-w64-clang-x86_64-http-parser-2.9.4-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.5.2-1-any.pkg.tar.zst";
  sha256 = "0zz4cq8ragsmsr87jplrdjai3rc0pz7kkixps17ppx4s4r7bvwmb";
  name = "mingw-w64-clang-x86_64-openssl-3.5.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libssh2-1.11.1-1-any.pkg.tar.zst";
  sha256 = "01l23cn5brficjzba7ldscqkdvk4rdcvvdyybd90qr2hqzligmhn";
  name = "mingw-w64-clang-x86_64-libssh2-1.11.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-bzip2-1.0.8-3-any.pkg.tar.zst";
  sha256 = "1n8zf2kk1xj7wiszp6mjchy1yzpalddbj0cj17qm625ags2vzflm";
  name = "mingw-w64-clang-x86_64-bzip2-1.0.8-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-wineditline-2.208-1-any.pkg.tar.zst";
  sha256 = "0x9d1ax81p0k1863z0jdhqg7454hb0svvil1f4aiqy1vlhqshfl5";
  name = "mingw-w64-clang-x86_64-wineditline-2.208-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pcre2-10.45-1-any.pkg.tar.zst";
  sha256 = "1bczcjb46wiphnlaarfjd78k1v9x1vnr9b7gq8xwib18hmzick9r";
  name = "mingw-w64-clang-x86_64-pcre2-10.45-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libgit2-1.9.1-1-any.pkg.tar.zst";
  sha256 = "024qw2y1r2rj99c891lb36vshsc8cngw9115l0y5jkf472b92ww9";
  name = "mingw-w64-clang-x86_64-libgit2-1.9.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
  sha256 = "17ha468qavwin800cc3b7c3xdggwk2gakasfxg7jdx7616d99l0n";
  name = "mingw-w64-clang-x86_64-termcap-1.3.1-7-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-readline-8.3.001-1-any.pkg.tar.zst";
  sha256 = "182j0v3vppp04wg84n2qp4zakpby92a3jah9m0jy780pbfvncqyc";
  name = "mingw-w64-clang-x86_64-readline-8.3.001-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-sqlite3-3.50.4-1-any.pkg.tar.zst";
  sha256 = "02j85x139926wx7f35cvy4s3avj0ii60hc5rdi7j7p6zfx2q64jm";
  name = "mingw-w64-clang-x86_64-sqlite3-3.50.4-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.89.0-3-any.pkg.tar.zst";
  sha256 = "1mawqq6954kznidhcch14af8qi6n8kp2rmng0psrm3062ghk84p0";
  name = "mingw-w64-clang-x86_64-rust-1.89.0-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
  sha256 = "0phhwkcqp30dsyj5vr6w99sgm1jfm5rzg0w5x5mv9md4x7lm9lmh";
  name = "mingw-w64-clang-x86_64-cppdap-1.65-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-c-ares-1.34.5-1-any.pkg.tar.zst";
  sha256 = "0r5kinyb90l0fr74zz7a5sn9qa4mgmz4j33azgyz1xx7zmchq5mh";
  name = "mingw-w64-clang-x86_64-c-ares-1.34.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-brotli-1.1.0-5-any.pkg.tar.zst";
  sha256 = "0safp8vwmn1190hjnhndhhw3iwailqnaypwhn2gifd40p6vgdci2";
  name = "mingw-w64-clang-x86_64-brotli-1.1.0-5-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunistring-1.3-1-any.pkg.tar.zst";
  sha256 = "1zg58qbfybyqzcj0dalb13l48f9jsras318h02rka65r7wi0pdcg";
  name = "mingw-w64-clang-x86_64-libunistring-1.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libidn2-2.3.8-2-any.pkg.tar.zst";
  sha256 = "109wmjiihw9s9dvyhn6wv92m4d8s7g9zq0d2c6z7j5vx2pffpyhp";
  name = "mingw-w64-clang-x86_64-libidn2-2.3.8-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20250419-1-any.pkg.tar.zst";
  sha256 = "1nhnqyh5wxlzg60nh1i9fcadgwsbln0vkgff1y8cn3fkxap15lxb";
  name = "mingw-w64-clang-x86_64-ca-certificates-20250419-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.66.0-1-any.pkg.tar.zst";
  sha256 = "1b7fsns0gjzf9vi3k1s92w5mm6521kyn3c1jhgn9zmmy4s8yww21";
  name = "mingw-w64-clang-x86_64-nghttp2-1.66.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gmp-6.3.0-2-any.pkg.tar.zst";
  sha256 = "03j72zks06pbwqbwsmv84f1441c333gy0k7d1yxzds95diyggwk9";
  name = "mingw-w64-clang-x86_64-gmp-6.3.0-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nettle-3.10.2-1-any.pkg.tar.zst";
  sha256 = "13gfzr5dfxcr5m050g3irwqk88lald25l0gybz130wnajp6sk0g8";
  name = "mingw-w64-clang-x86_64-nettle-3.10.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-gnutls-3.8.10-1-any.pkg.tar.zst";
  sha256 = "00p9kmp1pf9spc9jafwssg55p8znmif7d5h5a4rqckci1z9hamla";
  name = "mingw-w64-clang-x86_64-gnutls-3.8.10-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ngtcp2-1.14.0-1-any.pkg.tar.zst";
  sha256 = "14l98sijz2n57x7vi8bxhgaa8nn9n2wzpxzhwbx38wa4klbrqnml";
  name = "mingw-w64-clang-x86_64-ngtcp2-1.14.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.11.0-1-any.pkg.tar.zst";
  sha256 = "0s3dybqg6gmmkaab10bdm53nv10gwvmpfh0m2nbasgghylnn74yr";
  name = "mingw-w64-clang-x86_64-nghttp3-1.11.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.15.0-1-any.pkg.tar.zst";
  sha256 = "1cs5r8qzd035dwpv7wvbpirb2y8jvbjpq4jz3zqb33w2035qwqi2";
  name = "mingw-w64-clang-x86_64-curl-8.15.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.7.1-2-any.pkg.tar.zst";
  sha256 = "0b8sd94i2g1dbjpv5jhkayb42h015l2hn85yr2kyq3grr4xhx0sc";
  name = "mingw-w64-clang-x86_64-expat-2.7.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-jsoncpp-1.9.6-3-any.pkg.tar.zst";
  sha256 = "1ipilhiza17vz5dhgi61l80w2klw9f21w6jbyhi9wmfd6nxqv13c";
  name = "mingw-w64-clang-x86_64-jsoncpp-1.9.6-3-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libsystre-1.0.2-1-any.pkg.tar.zst";
  sha256 = "1r2ikm0jzziv6qjcjfv2mqiswzzr6css8vyp2syrzjvchy2ngl6y";
  name = "mingw-w64-clang-x86_64-libsystre-1.0.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libarchive-3.8.1-2-any.pkg.tar.zst";
  sha256 = "1zvjckps48ca9hi9cm5w68v2a41nsfhlhrc8d2mipn3h5c3y81ac";
  name = "mingw-w64-clang-x86_64-libarchive-3.8.1-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libuv-1.51.0-1-any.pkg.tar.zst";
  sha256 = "1bh8fjsqb15abvcxay2pyk741jb88ygjgqah0s9fcbc7wfj4p1br";
  name = "mingw-w64-clang-x86_64-libuv-1.51.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ninja-1.13.0-1-any.pkg.tar.zst";
  sha256 = "1wx1sp118rj1c7ylqmajfichra75h2hay2z061sg7v0az5lblr3n";
  name = "mingw-w64-clang-x86_64-ninja-1.13.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pkgconf-1~2.5.1-1-any.pkg.tar.zst";
  sha256 = "1srsggda5rkwsif82jrfxskvb10ix2nw38xk0nc7jpnf0ab529bb";
  name = "mingw-w64-clang-x86_64-pkgconf-12.5.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rhash-1.4.5-1-any.pkg.tar.zst";
  sha256 = "0gdn1351knjwgsqgyaa3l55qs135k7dn6mlf04vzjxlc1895wx5z";
  name = "mingw-w64-clang-x86_64-rhash-1.4.5-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-4.1.0-1-any.pkg.tar.zst";
  sha256 = "0qxw04ndl9s8dqna9hcdia7706s1hzj8fl5p26wmlzr3ysd5p5vg";
  name = "mingw-w64-clang-x86_64-cmake-4.1.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-mpdecimal-4.0.1-1-any.pkg.tar.zst";
  sha256 = "17nk5cj3rfsi82kay359kalrajf0qmi70innvr6h36g5d57mnwf4";
  name = "mingw-w64-clang-x86_64-mpdecimal-4.0.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ncurses-6.5.20241228-3-any.pkg.tar.zst";
  sha256 = "0f98pzrwsxil90n55hz2ym2x2rzrrjrmnj8i2203n189qbxbg2c9";
  name = "mingw-w64-clang-x86_64-ncurses-6.5.20241228-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tcl-8.6.16-1-any.pkg.tar.zst";
  sha256 = "1q72xa65sz5sj5z17fasnd5fifb4kfcn8jdjx83311k3k21gcvzn";
  name = "mingw-w64-clang-x86_64-tcl-8.6.16-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tk-8.6.16-1-any.pkg.tar.zst";
  sha256 = "0aa17ycq707fg2h8wvfv1vsfhwidc2z9zv7izi1cqq4bilmb29dr";
  name = "mingw-w64-clang-x86_64-tk-8.6.16-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-tzdata-2025b-2-any.pkg.tar.zst";
  sha256 = "04r54rzpf0vlpbidywshwb6l2lyql6wqbwcgplpicpkxgbcilpsa";
  name = "mingw-w64-clang-x86_64-tzdata-2025b-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.12.11-1-any.pkg.tar.zst";
  sha256 = "1smhrvizcrrvzp9bc69iiicjcxff8khsja0pk8rcxlf817s7z9j6";
  name = "mingw-w64-clang-x86_64-python-3.12.11-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-20.1.8-1-any.pkg.tar.zst";
  sha256 = "0sd9wvrrdv2galrpx3lhmi811qzbzgidykqyapjqs76447w5lid9";
  name = "mingw-w64-clang-x86_64-llvm-openmp-20.1.8-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.30-2-any.pkg.tar.zst";
  sha256 = "01x3gj0yk6x48nc3f0rg0gzl246sv2cidvrjhvy9ic8x1wl69xmd";
  name = "mingw-w64-clang-x86_64-openblas-0.3.30-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.3.2-1-any.pkg.tar.zst";
  sha256 = "13sl63k50x1nrimxbd1ml7f7lgblqbppl4pv2yqqbrwa9z6k2ycz";
  name = "mingw-w64-clang-x86_64-python-numpy-2.3.2-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-80.9.0-2-any.pkg.tar.zst";
  sha256 = "0vq9fw6dm8cvf835hd5aqyjq6bfcdcwmj7k1xdm5v09bdbr2cf7b";
  name = "mingw-w64-clang-x86_64-python-setuptools-80.9.0-2-any.pkg.tar.zst";
})
]
