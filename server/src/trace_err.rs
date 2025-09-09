pub trait TraceErr {
    fn trace_err(self) -> Self;
}

impl<T, E> TraceErr for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn trace_err(self) -> Self {
        self.inspect_err(|e| tracing::error!("{e:?}"))
    }
}
