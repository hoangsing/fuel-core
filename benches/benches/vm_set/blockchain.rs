use std::iter::successors;

use super::run_group_ref;

use criterion::{
    Criterion,
    Throughput,
};
use fuel_core::database::vm_database::VmDatabase;
use fuel_core_benches::*;
use fuel_core_storage::ContractsAssetsStorage;
use fuel_core_types::{
    fuel_asm::{
        op,
        GTFArgs,
        RegId,
    },
    fuel_tx::{
        Input,
        Output,
        Word,
    },
    fuel_types::*,
    fuel_vm::{
        consts::*,
        InterpreterStorage,
    },
};
use rand::{
    rngs::StdRng,
    RngCore,
    SeedableRng,
};

pub fn run(c: &mut Criterion) {
    let rng = &mut StdRng::seed_from_u64(2322u64);

    let mut linear: Vec<u64> = vec![1, 10, 100, 1000, 10_000];
    let mut l = successors(Some(100_000.0f64), |n| Some(n / 1.5))
        .take(5)
        .map(|f| f as u64)
        .collect::<Vec<_>>();
    l.sort_unstable();
    linear.extend(l);
    let asset: AssetId = rng.gen();
    let contract: ContractId = rng.gen();

    run_group_ref(
        &mut c.benchmark_group("bal"),
        "bal",
        VmBench::new(op::bal(0x10, 0x10, 0x11))
            .with_data(asset.iter().chain(contract.iter()).copied().collect())
            .with_prepare_script(vec![
                op::gtf_args(0x10, 0x00, GTFArgs::ScriptData),
                op::addi(0x11, 0x10, asset.len().try_into().unwrap()),
            ])
            .with_dummy_contract(contract)
            .with_prepare_db(move |mut db| {
                let mut asset_inc = AssetId::zeroed();

                asset_inc.as_mut()[..8].copy_from_slice(&1_u64.to_be_bytes());

                db.merkle_contract_asset_id_balance_insert(&contract, &asset_inc, 1)?;

                db.merkle_contract_asset_id_balance_insert(&contract, &asset, 100)?;

                Ok(db)
            }),
    );

    run_group_ref(
        &mut c.benchmark_group("sww"),
        "sww",
        VmBench::contract(rng, op::sww(RegId::ZERO, 0x29, RegId::ONE))
            .expect("failed to prepare contract")
            .with_prepare_db(move |mut db| {
                let mut key = Bytes32::zeroed();

                key.as_mut()[..8].copy_from_slice(&1_u64.to_be_bytes());

                db.merkle_contract_state_insert(&contract, &key, &key)?;

                Ok(db)
            }),
    );
    {
        let mut input = VmBench::contract(rng, op::srw(0x13, 0x14, 0x15))
            .expect("failed to prepare contract")
            .with_prepare_db(move |mut db| {
                let key = Bytes32::zeroed();

                db.merkle_contract_state_insert(&ContractId::zeroed(), &key, &key)?;

                Ok(db)
            });
        input.prepare_script.extend(vec![op::movi(0x15, 2000)]);
        run_group_ref(&mut c.benchmark_group("srw"), "srw", input);
    }

    let mut scwq = c.benchmark_group("scwq");

    for i in linear.clone() {
        let start_key = Bytes32::zeroed();
        let data = start_key.iter().copied().collect::<Vec<_>>();

        let post_call = vec![
            op::gtf_args(0x10, 0x00, GTFArgs::ScriptData),
            op::addi(0x11, 0x10, ContractId::LEN.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::movi(0x12, i as u32),
        ];
        let mut bench = VmBench::contract(rng, op::scwq(0x11, 0x29, 0x12))
            .expect("failed to prepare contract")
            .with_post_call(post_call)
            .with_prepare_db(move |mut db| {
                let slots = (0u64..i).map(|key_number| {
                    let mut key = Bytes32::zeroed();
                    key.as_mut()[..8].copy_from_slice(&key_number.to_be_bytes());
                    (key, key)
                });
                db.database_mut()
                    .init_contract_state(&contract, slots)
                    .unwrap();

                Ok(db)
            });
        bench.data.extend(data);

        scwq.throughput(Throughput::Bytes(i));

        run_group_ref(&mut scwq, format!("{i}"), bench);
    }

    scwq.finish();

    let mut swwq = c.benchmark_group("swwq");

    for i in linear.clone() {
        let start_key = Bytes32::zeroed();
        let data = start_key.iter().copied().collect::<Vec<_>>();

        let post_call = vec![
            op::gtf_args(0x10, 0x00, GTFArgs::ScriptData),
            op::addi(0x11, 0x10, ContractId::LEN.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::movi(0x12, i as u32),
        ];
        let mut bench = VmBench::contract(rng, op::swwq(0x10, 0x11, 0x20, 0x12))
            .expect("failed to prepare contract")
            .with_post_call(post_call)
            .with_prepare_db(move |mut db| {
                let slots = (0u64..i).map(|key_number| {
                    let mut key = Bytes32::zeroed();
                    key.as_mut()[..8].copy_from_slice(&key_number.to_be_bytes());
                    (key, key)
                });
                db.database_mut()
                    .init_contract_state(&contract, slots)
                    .unwrap();

                Ok(db)
            });
        bench.data.extend(data);

        swwq.throughput(Throughput::Bytes(i));

        run_group_ref(&mut swwq, format!("{i}"), bench);
    }

    swwq.finish();

    let mut call = c.benchmark_group("call");

    for i in linear.clone() {
        let mut code = vec![0u8; i as usize];

        rng.fill_bytes(&mut code);

        let code = ContractCode::from(code);
        let id = code.id;

        let data = id
            .iter()
            .copied()
            .chain((0 as Word).to_be_bytes().iter().copied())
            .chain((0 as Word).to_be_bytes().iter().copied())
            .chain(AssetId::default().iter().copied())
            .collect();

        let prepare_script = vec![
            op::gtf_args(0x10, 0x00, GTFArgs::ScriptData),
            op::addi(0x11, 0x10, ContractId::LEN.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::movi(0x12, 100_000),
        ];

        call.throughput(Throughput::Bytes(i));

        run_group_ref(
            &mut call,
            format!("{i}"),
            VmBench::new(op::call(0x10, RegId::ZERO, 0x11, 0x12))
                .with_contract_code(code)
                .with_data(data)
                .with_prepare_script(prepare_script),
        );
    }

    call.finish();

    let mut ldc = c.benchmark_group("ldc");

    for i in linear.clone() {
        let mut code = vec![0u8; i as usize];

        rng.fill_bytes(&mut code);

        let code = ContractCode::from(code);
        let id = code.id;

        let data = id
            .iter()
            .copied()
            .chain((0 as Word).to_be_bytes().iter().copied())
            .chain((0 as Word).to_be_bytes().iter().copied())
            .chain(AssetId::default().iter().copied())
            .collect();

        let prepare_script = vec![
            op::gtf_args(0x10, 0x00, GTFArgs::ScriptData),
            op::addi(0x11, 0x10, ContractId::LEN.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::movi(0x12, 100_000),
            op::movi(0x13, i.try_into().unwrap()),
        ];

        ldc.throughput(Throughput::Bytes(i));

        run_group_ref(
            &mut ldc,
            format!("{i}"),
            VmBench::new(op::ldc(0x10, RegId::ZERO, 0x13))
                .with_contract_code(code)
                .with_data(data)
                .with_prepare_script(prepare_script),
        );
    }

    ldc.finish();

    let mut ccp = c.benchmark_group("ccp");

    for i in linear.clone() {
        let mut code = vec![0u8; i as usize];

        rng.fill_bytes(&mut code);

        let code = ContractCode::from(code);
        let id = code.id;

        let data = id
            .iter()
            .copied()
            .chain((0 as Word).to_be_bytes().iter().copied())
            .chain((0 as Word).to_be_bytes().iter().copied())
            .chain(AssetId::default().iter().copied())
            .collect();

        let prepare_script = vec![
            op::gtf_args(0x10, 0x00, GTFArgs::ScriptData),
            op::addi(0x11, 0x10, ContractId::LEN.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::addi(0x11, 0x11, WORD_SIZE.try_into().unwrap()),
            op::movi(0x12, 100_000),
            op::movi(0x13, i.try_into().unwrap()),
            op::movi(0x14, i.try_into().unwrap()),
            op::movi(0x15, i.try_into().unwrap()),
            op::add(0x15, 0x15, 0x15),
            op::addi(0x15, 0x15, 32),
            op::aloc(0x15),
            op::move_(0x15, RegId::HP),
        ];

        ccp.throughput(Throughput::Bytes(i));

        run_group_ref(
            &mut ccp,
            format!("{i}"),
            VmBench::new(op::ccp(0x15, 0x10, RegId::ZERO, 0x13))
                .with_contract_code(code)
                .with_data(data)
                .with_prepare_script(prepare_script),
        );
    }

    ccp.finish();

    let mut csiz = c.benchmark_group("csiz");

    for i in linear.clone() {
        let mut code = vec![0u8; i as usize];

        rng.fill_bytes(&mut code);

        let code = ContractCode::from(code);
        let id = code.id;

        let data = id.iter().copied().collect();

        let prepare_script = vec![op::gtf_args(0x10, 0x00, GTFArgs::ScriptData)];

        csiz.throughput(Throughput::Bytes(i));

        run_group_ref(
            &mut csiz,
            format!("{i}"),
            VmBench::new(op::csiz(0x11, 0x10))
                .with_contract_code(code)
                .with_data(data)
                .with_prepare_script(prepare_script),
        );
    }

    csiz.finish();

    run_group_ref(
        &mut c.benchmark_group("bhei"),
        "bhei",
        VmBench::new(op::bhei(0x10)),
    );

    run_group_ref(
        &mut c.benchmark_group("bhsh"),
        "bhsh",
        VmBench::new(op::bhsh(0x10, RegId::ZERO)).with_prepare_script(vec![
            op::movi(0x10, Bytes32::LEN.try_into().unwrap()),
            op::aloc(0x10),
            op::move_(0x10, RegId::HP),
        ]),
    );

    run_group_ref(
        &mut c.benchmark_group("mint"),
        "mint",
        VmBench::contract(rng, op::mint(RegId::ZERO, RegId::ZERO))
            .expect("failed to prepare contract"),
    );

    run_group_ref(
        &mut c.benchmark_group("burn"),
        "burn",
        VmBench::contract(rng, op::mint(RegId::ZERO, RegId::ZERO))
            .expect("failed to prepare contract"),
    );

    run_group_ref(
        &mut c.benchmark_group("cb"),
        "cb",
        VmBench::new(op::cb(0x10)).with_prepare_script(vec![
            op::movi(0x10, Bytes32::LEN.try_into().unwrap()),
            op::aloc(0x10),
            op::move_(0x10, RegId::HP),
        ]),
    );

    {
        let mut input = VmBench::contract(rng, op::tr(0x15, 0x14, 0x15))
            .expect("failed to prepare contract")
            .with_prepare_db(move |mut db| {
                db.merkle_contract_asset_id_balance_insert(
                    &ContractId::zeroed(),
                    &AssetId::zeroed(),
                    200,
                )?;

                Ok(db)
            });
        input
            .prepare_script
            .extend(vec![op::movi(0x15, 2000), op::movi(0x14, 100)]);
        run_group_ref(&mut c.benchmark_group("tr"), "tr", input);
    }

    {
        let mut input = VmBench::contract(rng, op::tro(0x15, 0x16, 0x14, 0x15))
            .expect("failed to prepare contract")
            .with_prepare_db(move |mut db| {
                db.merkle_contract_asset_id_balance_insert(
                    &ContractId::zeroed(),
                    &AssetId::zeroed(),
                    200,
                )?;

                Ok(db)
            });
        let coin_output = Output::variable(Address::zeroed(), 100, AssetId::zeroed());
        input.outputs.push(coin_output);
        let predicate = op::ret(RegId::ONE).to_bytes().to_vec();
        let owner = Input::predicate_owner(&predicate);
        let coin_input = Input::coin_predicate(
            Default::default(),
            owner,
            1000,
            AssetId::zeroed(),
            Default::default(),
            Default::default(),
            Default::default(),
            predicate,
            vec![],
        );
        input.inputs.push(coin_input);

        let index = input.outputs.len() - 1;
        input.prepare_script.extend(vec![
            op::movi(0x15, 2000),
            op::movi(0x14, 100),
            op::movi(0x16, index.try_into().unwrap()),
        ]);
        run_group_ref(&mut c.benchmark_group("tro"), "tro", input);
    }

    run_group_ref(
        &mut c.benchmark_group("cfsi"),
        "cfsi",
        VmBench::new(op::cfsi(0)),
    );

    {
        let mut input = VmBench::contract(rng, op::croo(0x14, 0x16))
            .expect("failed to prepare contract");
        input.post_call.extend(vec![
            op::gtf_args(0x16, 0x00, GTFArgs::ScriptData),
            op::movi(0x15, 2000),
            op::aloc(0x15),
            op::move_(0x14, RegId::HP),
        ]);
        run_group_ref(&mut c.benchmark_group("croo"), "croo", input);
    }

    run_group_ref(
        &mut c.benchmark_group("flag"),
        "flag",
        VmBench::new(op::flag(0x10)),
    );

    run_group_ref(
        &mut c.benchmark_group("gm"),
        "gm",
        VmBench::contract(rng, op::gm(0x10, 1)).unwrap(),
    );

    let mut smo = c.benchmark_group("smo");

    for i in linear.clone() {
        let mut input = VmBench::contract(rng, op::smo(0x15, 0x16, 0x17, 0x18))
            .expect("failed to prepare contract");
        input.prepare_db = Some(Box::new(|mut db: VmDatabase| {
            db.merkle_contract_asset_id_balance_insert(
                &ContractId::default(),
                &AssetId::default(),
                Word::MAX,
            )?;
            Ok(db)
        }));
        input.post_call.extend(vec![
            op::gtf_args(0x15, 0x00, GTFArgs::ScriptData),
            // Offset 32 + 8 + 8 + 32
            op::addi(0x15, 0x15, 32 + 8 + 8 + 32), // target address pointer
            op::addi(0x16, 0x15, 32),              // data ppinter
            op::movi(0x17, i.try_into().unwrap()), // data length
            op::movi(0x18, 10),                    // coins to send
        ]);
        input.data.extend(
            Address::new([1u8; 32])
                .iter()
                .copied()
                .chain(vec![2u8; i as usize]),
        );
        let predicate = op::ret(RegId::ONE).to_bytes().to_vec();
        let owner = Input::predicate_owner(&predicate);
        let coin_input = Input::coin_predicate(
            Default::default(),
            owner,
            Word::MAX,
            AssetId::zeroed(),
            Default::default(),
            Default::default(),
            Default::default(),
            predicate,
            vec![],
        );
        input.inputs.push(coin_input);
        smo.throughput(Throughput::Bytes(i));
        run_group_ref(&mut smo, format!("{i}"), input);
    }

    smo.finish();

    let mut srwq = c.benchmark_group("srwq");

    for i in linear.clone() {
        let start_key = Bytes32::zeroed();
        let data = start_key.iter().copied().collect::<Vec<_>>();

        let post_call = vec![
            op::movi(0x16, i as u32),
            op::movi(0x17, 2000),
            op::move_(0x15, 0x16),
            op::muli(0x15, 0x15, 32),
            op::addi(0x15, 0x15, 1),
            op::aloc(0x15),
            op::move_(0x14, RegId::HP),
        ];
        let mut bench = VmBench::contract(rng, op::srwq(0x14, 0x11, 0x27, 0x16))
            .expect("failed to prepare contract")
            .with_post_call(post_call)
            .with_prepare_db(move |mut db| {
                let slots = (0u64..i).map(|key_number| {
                    let mut key = Bytes32::zeroed();
                    key.as_mut()[..8].copy_from_slice(&key_number.to_be_bytes());
                    (key, key)
                });
                db.database_mut()
                    .init_contract_state(&contract, slots)
                    .unwrap();

                Ok(db)
            });
        bench.data.extend(data);
        srwq.throughput(Throughput::Bytes(i));
        run_group_ref(&mut srwq, format!("{i}"), bench);
    }

    srwq.finish();

    run_group_ref(
        &mut c.benchmark_group("time"),
        "time",
        VmBench::new(op::time(0x11, 0x10)).with_prepare_script(vec![op::movi(0x10, 0)]),
    );
}
