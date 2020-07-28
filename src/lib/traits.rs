pub trait DataManager {
    type Data: Ord + PartialOrd;

    /// Starts the main program for the manager.
    /// Returns an integer that should be interpreted as the application exit code.
    fn start() -> i32;

    /// Returns an immutable reference to the data inside the manager.
    fn data(&self) -> &Vec<Self::Data>;

    /// Returns a mutable reference to the data inside the manager.
    fn data_mut(&mut self) -> &mut Vec<Self::Data>;
}
