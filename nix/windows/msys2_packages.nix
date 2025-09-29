{ pkgs } : [

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libunwind-21.1.1-1-any.pkg.tar.zst";
  sha256 = "0m3a5b6pi6pqc9kjl453kkfbpbw9hixczq1i35ahwgglczw60z3p";
  name = "mingw-w64-clang-x86_64-libunwind-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libc++-21.1.1-1-any.pkg.tar.zst";
  sha256 = "105qhspr2hy9h887b9gvcf3x5r01w8ggi0526jr42r81wyy5hg90";
  name = "mingw-w64-clang-x86_64-libc++-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libffi-3.5.2-1-any.pkg.tar.zst";
  sha256 = "02lc36mk43vi6lg4gb4dkyigk56fkqdk7b3ycapmih1w7kfyqq2r";
  name = "mingw-w64-clang-x86_64-libffi-3.5.2-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libxml2-2.14.6-1-any.pkg.tar.zst";
  sha256 = "14r2p36qqnc8aq3gf6rz3mrszckhkfi9vd1q8v67a4isfdx1bb66";
  name = "mingw-w64-clang-x86_64-libxml2-2.14.6-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
  sha256 = "1hrx54k2s3dcs8fhwdwms5amr4gjid1d20b2b4302xyjg9yyvpxl";
  name = "mingw-w64-clang-x86_64-zstd-1.5.7-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-libs-21.1.1-1-any.pkg.tar.zst";
  sha256 = "03s0swygziqa4gf09p51d3nnmsb2alfzrmjb4mrkvfmd9n4m0zca";
  name = "mingw-w64-clang-x86_64-llvm-libs-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-libs-21.1.1-1-any.pkg.tar.zst";
  sha256 = "1hvpkj07bfsw9lppavj9jjjvrwis1j9pb39yrdan54483x7yyvhh";
  name = "mingw-w64-clang-x86_64-clang-libs-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-compiler-rt-21.1.1-1-any.pkg.tar.zst";
  sha256 = "0lbqdq6rzf65a02lkz9qch24db4mrlgf8f62d3agqn1pyqwby3da";
  name = "mingw-w64-clang-x86_64-compiler-rt-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-tools-21.1.1-1-any.pkg.tar.zst";
  sha256 = "0mv34np81ailsbc7mzpw7z5xywy77nkkjk8202dsvdsayiby702h";
  name = "mingw-w64-clang-x86_64-llvm-tools-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-headers-git-13.0.0.r179.g8181947cc-1-any.pkg.tar.zst";
  sha256 = "1fi6bcgbx5080z3v1av39kn5wnybgyjx6aggahxhrb20mgq9i8mx";
  name = "mingw-w64-clang-x86_64-headers-git-13.0.0.r179.g8181947cc-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-crt-git-13.0.0.r179.g8181947cc-1-any.pkg.tar.zst";
  sha256 = "0bzzfscl2nyv81w3inkkl7f7injsgpcgbbivm3nm3zyra1ka0clf";
  name = "mingw-w64-clang-x86_64-crt-git-13.0.0.r179.g8181947cc-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-lld-21.1.1-1-any.pkg.tar.zst";
  sha256 = "0an9dzbrg75fmm6bp9hls5ailcd9ngvhma2jcvwcyznv6x8pps8h";
  name = "mingw-w64-clang-x86_64-lld-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libwinpthread-13.0.0.r179.g8181947cc-1-any.pkg.tar.zst";
  sha256 = "0mgxz784hbazmcwd3s44b7m2v38d07cq5yc7cdznrjmiypp8hwk3";
  name = "mingw-w64-clang-x86_64-libwinpthread-13.0.0.r179.g8181947cc-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-winpthreads-13.0.0.r179.g8181947cc-1-any.pkg.tar.zst";
  sha256 = "0yir5d351jqixvx5jrgsvyqlmsjgl97icx24215xzkbf5z2xlpg6";
  name = "mingw-w64-clang-x86_64-winpthreads-13.0.0.r179.g8181947cc-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-clang-21.1.1-1-any.pkg.tar.zst";
  sha256 = "1wywhprrl2nnn4gz6mq4d57i47x5khc8n6sn03jya4s1fvrjhnvn";
  name = "mingw-w64-clang-x86_64-clang-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-http-parser-2.9.4-2-any.pkg.tar.zst";
  sha256 = "0bmnpq7cqihspyma4xxg3rsz4z8wqc294pi7wfy2vxn26m82rfy5";
  name = "mingw-w64-clang-x86_64-http-parser-2.9.4-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openssl-3.5.3-1-any.pkg.tar.zst";
  sha256 = "1m10925vnkfkq4lhdd075alkbn8h0bd13ha8lak1fynpi91vrc4m";
  name = "mingw-w64-clang-x86_64-openssl-3.5.3-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-pcre2-10.46-1-any.pkg.tar.zst";
  sha256 = "122q5mwa156va0ji9v8gmn501k15sy08d3biji0rnryprl308pfd";
  name = "mingw-w64-clang-x86_64-pcre2-10.46-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-rust-1.90.0-2-any.pkg.tar.zst";
  sha256 = "0gfiicqar1waznj3772sdi6bpz1aws2ncdkjdh4f4pdbpbdk3psj";
  name = "mingw-w64-clang-x86_64-rust-1.90.0-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libidn2-2.3.8-3-any.pkg.tar.zst";
  sha256 = "1kjbpj4dbk7xlsx85wzw0y9fq46y1b9vk8mg2qpdyrihg5503dfn";
  name = "mingw-w64-clang-x86_64-libidn2-2.3.8-3-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-p11-kit-0.25.10-1-any.pkg.tar.zst";
  sha256 = "1vwlyv5c52hmc2madb8rw0s30msncymawh2vgaiy648x7dsfbwi6";
  name = "mingw-w64-clang-x86_64-p11-kit-0.25.10-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ca-certificates-20250419-1-any.pkg.tar.zst";
  sha256 = "1nhnqyh5wxlzg60nh1i9fcadgwsbln0vkgff1y8cn3fkxap15lxb";
  name = "mingw-w64-clang-x86_64-ca-certificates-20250419-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp2-1.67.1-1-any.pkg.tar.zst";
  sha256 = "15cncaybpq1m0qrf87i9h2zk1v8nmjwd8ccsqrzjb5c207m0dkc9";
  name = "mingw-w64-clang-x86_64-nghttp2-1.67.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ngtcp2-1.16.0-1-any.pkg.tar.zst";
  sha256 = "0akq2smhm7nahhmarwkrm657rw7v4499rf8s5l5ilamf95sh8hpi";
  name = "mingw-w64-clang-x86_64-ngtcp2-1.16.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-nghttp3-1.12.0-1-any.pkg.tar.zst";
  sha256 = "1pj67igdfx9rpx1kkn56drigfha24z2qj80qj1xbi10ic1wq3v67";
  name = "mingw-w64-clang-x86_64-nghttp3-1.12.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-curl-8.16.0-1-any.pkg.tar.zst";
  sha256 = "0na53rws9xa1q1ddp3xndlq8hj9sjw6sm0syhh2kabli3q202rxp";
  name = "mingw-w64-clang-x86_64-curl-8.16.0-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-expat-2.7.3-1-any.pkg.tar.zst";
  sha256 = "04sp1xlv9hx39zhh5i557frmifgkk1lwx333iwbn33f061kmfz3h";
  name = "mingw-w64-clang-x86_64-expat-2.7.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-jsoncpp-1.9.6-3-any.pkg.tar.zst";
  sha256 = "1ipilhiza17vz5dhgi61l80w2klw9f21w6jbyhi9wmfd6nxqv13c";
  name = "mingw-w64-clang-x86_64-jsoncpp-1.9.6-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-libb2-0.98.1-3-any.pkg.tar.zst";
  sha256 = "1qn2xlvv1xc3qyfqar3fdmg7mqfsvaa1x68y9jwbvar7xbqnjb3g";
  name = "mingw-w64-clang-x86_64-libb2-0.98.1-3-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-ninja-1.13.1-1-any.pkg.tar.zst";
  sha256 = "040ymqf8f3kpm6sgs2n7qqxinbsbkn0zc945l3hr9hyjfggxdvi5";
  name = "mingw-w64-clang-x86_64-ninja-1.13.1-1-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-cmake-4.1.1-2-any.pkg.tar.zst";
  sha256 = "1h92xy63a4zziyjxglm8k2kq0pi5vf8lcv9gcvdw6fy8c6d3fpl4";
  name = "mingw-w64-clang-x86_64-cmake-4.1.1-2-any.pkg.tar.zst";
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
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-3.12.11-3-any.pkg.tar.zst";
  sha256 = "18lajhbjmwdglhjlrkwb129xgaf3h1x046yzbs8ynmx1ar4vv2fi";
  name = "mingw-w64-clang-x86_64-python-3.12.11-3-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-llvm-openmp-21.1.1-1-any.pkg.tar.zst";
  sha256 = "0dv6kbc411flj88ijnpgr8lqnkblwfh74cfpw05zlx89cygzbw42";
  name = "mingw-w64-clang-x86_64-llvm-openmp-21.1.1-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-openblas-0.3.30-2-any.pkg.tar.zst";
  sha256 = "01x3gj0yk6x48nc3f0rg0gzl246sv2cidvrjhvy9ic8x1wl69xmd";
  name = "mingw-w64-clang-x86_64-openblas-0.3.30-2-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-numpy-2.3.3-1-any.pkg.tar.zst";
  sha256 = "0fjgl8b0a4mmnn3ql1nibvqhgl5zz4dwabqmjjgsk30jdirpxch1";
  name = "mingw-w64-clang-x86_64-python-numpy-2.3.3-1-any.pkg.tar.zst";
})

(pkgs.fetchurl {
  url = "https://mirror.msys2.org/mingw/clang64/mingw-w64-clang-x86_64-python-setuptools-80.9.0-2-any.pkg.tar.zst";
  sha256 = "0vq9fw6dm8cvf835hd5aqyjq6bfcdcwmj7k1xdm5v09bdbr2cf7b";
  name = "mingw-w64-clang-x86_64-python-setuptools-80.9.0-2-any.pkg.tar.zst";
})
]
