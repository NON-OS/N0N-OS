#[repr(u64)]
pub enum Sys {
    PRINT = 1,
    SEAL  = 2,
    EXEC  = 3,
}

/*   syscall numbers -> handler functions mapping   */
