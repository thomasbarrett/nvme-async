# nvme-async

## Example
The following minimal example shows how this library can be used to asynchronous block I/O
using the `io_uring` NVMe passthrough command.
```rust
    use std::rc::Rc;
    use io_uring::{squeue::Entry128, cqueue::Entry32};
    use io_uring_async::IoUringAsync;
    use send_wrapper::SendWrapper;
    use nvme_async::NvmeBlockDevice;

    fn main() {
        let uring: IoUringAsync<Entry128, Entry32> = IoUringAsync::generic_new(8).unwrap();
        let uring = Rc::new(uring);

        // Create a new current_thread runtime that submits all outstanding submission queue
        // entries as soon as the executor goes idle.
        let uring_clone = SendWrapper::new(uring.clone());
        let runtime = tokio::runtime::Builder::new_current_thread().
            on_thread_park(move || { uring_clone.submit().unwrap(); }).
            enable_all().
            build().unwrap();  

        runtime.block_on(async move {
            tokio::task::LocalSet::new().run_until(async {
                tokio::task::spawn_local(IoUringAsync::listen(uring.clone()));

                let bdev = NvmeBlockDevice::open("/dev/ng0n1", 1, uring).unwrap();
                
                let mut buf = vec![0u8; 512];
                let res = bdev.write_at(&mut buf, 0).await.unwrap();
                println!("{} bytes written: {:?}", res, buf);

                let mut buf = vec![0u8; 512];
                let res = bdev.read_at(&mut buf, 0).await.unwrap();
                println!("{} bytes read: {:?}", res, buf);
            }).await; 
        });
    }
```