pub mod ast;

pub mod synapse {
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "synapse.pest"]
    pub struct SynapseParser;
}

pub use synapse::SynapseParser;

#[cfg(test)]
mod grammar_tests;
