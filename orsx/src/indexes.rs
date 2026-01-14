#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexType {
    BTree,
    Hash,
    Gin,
    Gist,
}

impl IndexType {
    pub fn to_sql(&self) -> &'static str {
        match self {
            IndexType::BTree => "BTREE",
            IndexType::Hash => "HASH",
            IndexType::Gin => "GIN",
            IndexType::Gist => "GIST",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexInfo {
    pub name: &'static str,
    pub columns: &'static [&'static str],
    pub unique: bool,
    pub index_type: IndexType,
}
