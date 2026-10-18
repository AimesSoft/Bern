#[bern::session]
pub struct Draft {
    #[initial(other.name)]
    pub name: String,
}

fn main() {}
