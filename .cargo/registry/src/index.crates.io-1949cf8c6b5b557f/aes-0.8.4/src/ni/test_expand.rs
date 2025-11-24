use super::utils::check;
use hex_literal::hex;

#[test]
fn aes128_expand_key_test() {
    use super::aes128::expand_key;

    let keys = [0x00; 16];
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x0000000000000000, 0x0000000000000000],
            [0x6263636362636363, 0x6263636362636363],
            [0x9b9898c9f9fbfbaa, 0x9b9898c9f9fbfbaa],
            [0x90973450696ccffa, 0xf2f457330b0fac99],
            [0xee06da7b876a1581, 0x759e42b27e91ee2b],
            [0x7f2e2b88f8443e09, 0x8dda7cbbf34b9290],
            [0xec614b851425758c, 0x99ff09376ab49ba7],
            [0x217517873550620b, 0xacaf6b3cc61bf09b],
            [0x0ef903333ba96138, 0x97060a04511dfa9f],
            [0xb1d4d8e28a7db9da, 0x1d7bb3de4c664941],
            [0xb4ef5bcb3e92e211, 0x23e951cf6f8f188e],
        ],
    );

    let keys = [0xff; 16];
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0xffffffffffffffff, 0xffffffffffffffff],
            [0xe8e9e9e917161616, 0xe8e9e9e917161616],
            [0xadaeae19bab8b80f, 0x525151e6454747f0],
            [0x090e2277b3b69a78, 0xe1e7cb9ea4a08c6e],
            [0xe16abd3e52dc2746, 0xb33becd8179b60b6],
            [0xe5baf3ceb766d488, 0x045d385013c658e6],
            [0x71d07db3c6b6a93b, 0xc2eb916bd12dc98d],
            [0xe90d208d2fbb89b6, 0xed5018dd3c7dd150],
            [0x96337366b988fad0, 0x54d8e20d68a5335d],
            [0x8bf03f233278c5f3, 0x66a027fe0e0514a3],
            [0xd60a3588e472f07b, 0x82d2d7858cd7c326],
        ],
    );

    let keys = hex!("000102030405060708090a0b0c0d0e0f");
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x0001020304050607, 0x08090a0b0c0d0e0f],
            [0xd6aa74fdd2af72fa, 0xdaa678f1d6ab76fe],
            [0xb692cf0b643dbdf1, 0xbe9bc5006830b3fe],
            [0xb6ff744ed2c2c9bf, 0x6c590cbf0469bf41],
            [0x47f7f7bc95353e03, 0xf96c32bcfd058dfd],
            [0x3caaa3e8a99f9deb, 0x50f3af57adf622aa],
            [0x5e390f7df7a69296, 0xa7553dc10aa31f6b],
            [0x14f9701ae35fe28c, 0x440adf4d4ea9c026],
            [0x47438735a41c65b9, 0xe016baf4aebf7ad2],
            [0x549932d1f0855768, 0x1093ed9cbe2c974e],
            [0x13111d7fe3944a17, 0xf307a78b4d2b30c5],
        ],
    );

    let keys = hex!("6920e299a5202a6d656e636869746f2a");
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x6920e299a5202a6d, 0x656e636869746f2a],
            [0xfa8807605fa82d0d, 0x3ac64e6553b2214f],
            [0xcf75838d90ddae80, 0xaa1be0e5f9a9c1aa],
            [0x180d2f1488d08194, 0x22cb6171db62a0db],
            [0xbaed96ad323d1739, 0x10f67648cb94d693],
            [0x881b4ab2ba265d8b, 0xaad02bc36144fd50],
            [0xb34f195d096944d6, 0xa3b96f15c2fd9245],
            [0xa7007778ae6933ae, 0x0dd05cbbcf2dcefe],
            [0xff8bccf251e2ff5c, 0x5c32a3e7931f6d19],
            [0x24b7182e7555e772, 0x29674495ba78298c],
            [0xae127cdadb479ba8, 0xf220df3d4858f6b1],
        ],
    );

    let keys = hex!("2b7e151628aed2a6abf7158809cf4f3c");
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x2b7e151628aed2a6, 0xabf7158809cf4f3c],
            [0xa0fafe1788542cb1, 0x23a339392a6c7605],
            [0xf2c295f27a96b943, 0x5935807a7359f67f],
            [0x3d80477d4716fe3e, 0x1e237e446d7a883b],
            [0xef44a541a8525b7f, 0xb671253bdb0bad00],
            [0xd4d1c6f87c839d87, 0xcaf2b8bc11f915bc],
            [0x6d88a37a110b3efd, 0xdbf98641ca0093fd],
            [0x4e54f70e5f5fc9f3, 0x84a64fb24ea6dc4f],
            [0xead27321b58dbad2, 0x312bf5607f8d292f],
            [0xac7766f319fadc21, 0x28d12941575c006e],
            [0xd014f9a8c9ee2589, 0xe13f0cc8b6630ca6],
        ],
    );
}

#[test]
fn aes192_expand_key_test() {
    use super::aes192::expand_key;

    let keys = [0x00; 24];
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x0000000000000000, 0x0000000000000000],
            [0x0000000000000000, 0x6263636362636363],
            [0x6263636362636363, 0x6263636362636363],
            [0x9b9898c9f9fbfbaa, 0x9b9898c9f9fbfbaa],
            [0x9b9898c9f9fbfbaa, 0x90973450696ccffa],
            [0xf2f457330b0fac99, 0x90973450696ccffa],
            [0xc81d19a9a171d653, 0x53858160588a2df9],
            [0xc81d19a9a171d653, 0x7bebf49bda9a22c8],
            [0x891fa3a8d1958e51, 0x198897f8b8f941ab],
            [0xc26896f718f2b43f, 0x91ed1797407899c6],
            [0x59f00e3ee1094f95, 0x83ecbc0f9b1e0830],
            [0x0af31fa74a8b8661, 0x137b885ff272c7ca],
            [0x432ac886d834c0b6, 0xd2c7df11984c5970],
        ],
    );

    let keys = [0xff; 24];
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0xffffffffffffffff, 0xffffffffffffffff],
            [0xffffffffffffffff, 0xe8e9e9e917161616],
            [0xe8e9e9e917161616, 0xe8e9e9e917161616],
            [0xadaeae19bab8b80f, 0x525151e6454747f0],
            [0xadaeae19bab8b80f, 0xc5c2d8ed7f7a60e2],
            [0x2d2b3104686c76f4, 0xc5c2d8ed7f7a60e2],
            [0x1712403f686820dd, 0x454311d92d2f672d],
            [0xe8edbfc09797df22, 0x8f8cd3b7e7e4f36a],
            [0xa2a7e2b38f88859e, 0x67653a5ef0f2e57c],
            [0x2655c33bc1b13051, 0x6316d2e2ec9e577c],
            [0x8bfb6d227b09885e, 0x67919b1aa620ab4b],
            [0xc53679a929a82ed5, 0xa25343f7d95acba9],
            [0x598e482fffaee364, 0x3a989acd1330b418],
        ],
    );

    let keys = hex!("000102030405060708090a0b0c0d0e0f1011121314151617");
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x0001020304050607, 0x08090a0b0c0d0e0f],
            [0x1011121314151617, 0x5846f2f95c43f4fe],
            [0x544afef55847f0fa, 0x4856e2e95c43f4fe],
            [0x40f949b31cbabd4d, 0x48f043b810b7b342],
            [0x58e151ab04a2a555, 0x7effb5416245080c],
            [0x2ab54bb43a02f8f6, 0x62e3a95d66410c08],
            [0xf501857297448d7e, 0xbdf1c6ca87f33e3c],
            [0xe510976183519b69, 0x34157c9ea351f1e0],
            [0x1ea0372a99530916, 0x7c439e77ff12051e],
            [0xdd7e0e887e2fff68, 0x608fc842f9dcc154],
            [0x859f5f237a8d5a3d, 0xc0c02952beefd63a],
            [0xde601e7827bcdf2c, 0xa223800fd8aeda32],
            [0xa4970a331a78dc09, 0xc418c271e3a41d5d],
        ],
    );

    let keys = hex!("8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b");
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x8e73b0f7da0e6452, 0xc810f32b809079e5],
            [0x62f8ead2522c6b7b, 0xfe0c91f72402f5a5],
            [0xec12068e6c827f6b, 0x0e7a95b95c56fec2],
            [0x4db7b4bd69b54118, 0x85a74796e92538fd],
            [0xe75fad44bb095386, 0x485af05721efb14f],
            [0xa448f6d94d6dce24, 0xaa326360113b30e6],
            [0xa25e7ed583b1cf9a, 0x27f939436a94f767],
            [0xc0a69407d19da4e1, 0xec1786eb6fa64971],
            [0x485f703222cb8755, 0xe26d135233f0b7b3],
            [0x40beeb282f18a259, 0x6747d26b458c553e],
            [0xa7e1466c9411f1df, 0x821f750aad07d753],
            [0xca4005388fcc5006, 0x282d166abc3ce7b5],
            [0xe98ba06f448c773c, 0x8ecc720401002202],
        ],
    );
}

#[test]
fn aes256_expand_key_test() {
    use super::aes256::expand_key;

    let keys = [0x00; 32];
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x0000000000000000, 0x0000000000000000],
            [0x0000000000000000, 0x0000000000000000],
            [0x6263636362636363, 0x6263636362636363],
            [0xaafbfbfbaafbfbfb, 0xaafbfbfbaafbfbfb],
            [0x6f6c6ccf0d0f0fac, 0x6f6c6ccf0d0f0fac],
            [0x7d8d8d6ad7767691, 0x7d8d8d6ad7767691],
            [0x5354edc15e5be26d, 0x31378ea23c38810e],
            [0x968a81c141fcf750, 0x3c717a3aeb070cab],
            [0x9eaa8f28c0f16d45, 0xf1c6e3e7cdfe62e9],
            [0x2b312bdf6acddc8f, 0x56bca6b5bdbbaa1e],
            [0x6406fd52a4f79017, 0x553173f098cf1119],
            [0x6dbba90b07767584, 0x51cad331ec71792f],
            [0xe7b0e89c4347788b, 0x16760b7b8eb91a62],
            [0x74ed0ba1739b7e25, 0x2251ad14ce20d43b],
            [0x10f80a1753bf729c, 0x45c979e7cb706385],
        ],
    );

    let keys = [0xff; 32];
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0xffffffffffffffff, 0xffffffffffffffff],
            [0xffffffffffffffff, 0xffffffffffffffff],
            [0xe8e9e9e917161616, 0xe8e9e9e917161616],
            [0x0fb8b8b8f0474747, 0x0fb8b8b8f0474747],
            [0x4a4949655d5f5f73, 0xb5b6b69aa2a0a08c],
            [0x355858dcc51f1f9b, 0xcaa7a7233ae0e064],
            [0xafa80ae5f2f75596, 0x4741e30ce5e14380],
            [0xeca0421129bf5d8a, 0xe318faa9d9f81acd],
            [0xe60ab7d014fde246, 0x53bc014ab65d42ca],
            [0xa2ec6e658b5333ef, 0x684bc946b1b3d38b],
            [0x9b6c8a188f91685e, 0xdc2d69146a702bde],
            [0xa0bd9f782beeac97, 0x43a565d1f216b65a],
            [0xfc22349173b35ccf, 0xaf9e35dbc5ee1e05],
            [0x0695ed132d7b4184, 0x6ede24559cc8920f],
            [0x546d424f27de1e80, 0x88402b5b4dae355e],
        ],
    );

    let keys = hex!("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x0001020304050607, 0x08090a0b0c0d0e0f],
            [0x1011121314151617, 0x18191a1b1c1d1e1f],
            [0xa573c29fa176c498, 0xa97fce93a572c09c],
            [0x1651a8cd0244beda, 0x1a5da4c10640bade],
            [0xae87dff00ff11b68, 0xa68ed5fb03fc1567],
            [0x6de1f1486fa54f92, 0x75f8eb5373b8518d],
            [0xc656827fc9a79917, 0x6f294cec6cd5598b],
            [0x3de23a75524775e7, 0x27bf9eb45407cf39],
            [0x0bdc905fc27b0948, 0xad5245a4c1871c2f],
            [0x45f5a66017b2d387, 0x300d4d33640a820a],
            [0x7ccff71cbeb4fe54, 0x13e6bbf0d261a7df],
            [0xf01afafee7a82979, 0xd7a5644ab3afe640],
            [0x2541fe719bf50025, 0x8813bbd55a721c0a],
            [0x4e5a6699a9f24fe0, 0x7e572baacdf8cdea],
            [0x24fc79ccbf0979e9, 0x371ac23c6d68de36],
        ],
    );

    let keys = hex!("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
    check(
        unsafe { &expand_key(&keys) },
        &[
            [0x603deb1015ca71be, 0x2b73aef0857d7781],
            [0x1f352c073b6108d7, 0x2d9810a30914dff4],
            [0x9ba354118e6925af, 0xa51a8b5f2067fcde],
            [0xa8b09c1a93d194cd, 0xbe49846eb75d5b9a],
            [0xd59aecb85bf3c917, 0xfee94248de8ebe96],
            [0xb5a9328a2678a647, 0x983122292f6c79b3],
            [0x812c81addadf48ba, 0x24360af2fab8b464],
            [0x98c5bfc9bebd198e, 0x268c3ba709e04214],
            [0x68007bacb2df3316, 0x96e939e46c518d80],
            [0xc814e20476a9fb8a, 0x5025c02d59c58239],
            [0xde1369676ccc5a71, 0xfa2563959674ee15],
            [0x5886ca5d2e2f31d7, 0x7e0af1fa27cf73c3],
            [0x749c47ab18501dda, 0xe2757e4f7401905a],
            [0xcafaaae3e4d59b34, 0x9adf6acebd10190d],
            [0xfe4890d1e6188d0b, 0x046df344706c631e],
        ],
    );
}
