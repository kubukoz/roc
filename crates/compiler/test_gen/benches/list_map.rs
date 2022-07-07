#[path = "../src/helpers/mod.rs"]
mod helpers;

// defines roc_alloc and friends
pub use helpers::platform_functions::*;

use bumpalo::Bump;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use roc_gen_llvm::{run_roc::RocCallResult, run_roc_dylib};
use roc_mono::ir::OptLevel;
use roc_std::RocList;

// results July 6th, 2022
//
//    roc sum map             time:   [612.73 ns 614.24 ns 615.98 ns]
//    roc sum map_with_index  time:   [5.3177 us 5.3218 us 5.3255 us]
//    rust (debug)            time:   [24.081 us 24.163 us 24.268 us]

type Input = RocList<i64>;
type Output = i64;

type Main<I, O> = unsafe extern "C" fn(I, *mut RocCallResult<O>);

const ROC_LIST_MAP: &str = indoc::indoc!(
    r#"
    app "bench" provides [main] to "./platform"

    main : List I64 -> Nat 
    main = \list -> 
        list
            |> List.map (\x -> x + 2)
            |> List.len
    "#
);

const ROC_LIST_MAP_WITH_INDEX: &str = indoc::indoc!(
    r#"
    app "bench" provides [main] to "./platform"

    main : List I64 -> Nat 
    main = \list -> 
        list
            |> List.mapWithIndex (\x, _ -> x + 2)
            |> List.len
    "#
);

fn roc_function<'a, 'b>(
    arena: &'a Bump,
    source: &str,
) -> libloading::Symbol<'a, Main<&'b Input, Output>> {
    let config = helpers::llvm::HelperConfig {
        is_gen_test: true,
        ignore_problems: false,
        add_debug_info: true,
        opt_level: OptLevel::Optimize,
    };

    let context = inkwell::context::Context::create();
    let (main_fn_name, errors, lib) =
        helpers::llvm::helper(arena, config, source, arena.alloc(context));

    assert!(errors.is_empty(), "Encountered errors:\n{}", errors);

    run_roc_dylib!(arena.alloc(lib), main_fn_name, &Input, Output, errors)
}

fn rust_main(argument: &RocList<i64>, output: &mut RocCallResult<i64>) {
    let mut answer = 0;

    for x in argument.iter() {
        answer += x;
    }

    *output = RocCallResult::new(answer);
}

fn create_input_list() -> RocList<i64> {
    let numbers = Vec::from_iter(0..1_000);

    RocList::from_slice(&numbers)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let arena = Bump::new();

    let list_map_with_index_main = roc_function(&arena, ROC_LIST_MAP_WITH_INDEX);
    let list_map_main = roc_function(&arena, ROC_LIST_MAP);

    let input = &*arena.alloc(create_input_list());

    c.bench_function("roc sum map", |b| {
        b.iter(|| unsafe {
            let mut main_result = RocCallResult::default();

            list_map_main(black_box(input), &mut main_result);
        })
    });

    c.bench_function("roc sum map_with_index", |b| {
        b.iter(|| unsafe {
            let mut main_result = RocCallResult::default();

            list_map_with_index_main(black_box(input), &mut main_result);
        })
    });

    let input = &*arena.alloc(create_input_list());
    c.bench_function("rust", |b| {
        b.iter(|| {
            let mut main_result = RocCallResult::default();

            rust_main(black_box(input), &mut main_result);
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);