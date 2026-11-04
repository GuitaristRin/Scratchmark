using Scratchmark;

internal static unsafe partial class Program
{
    private static void NativeErrors(byte[] fixture)
    {
        Require(Native.GetState(ulong.MaxValue, out _) == Status.InvalidHandle, "64-bit invalid handle");
        Require(Native.Create(out ulong handle) == Status.Ok && handle != 0, "raw create");
        try
        {
            fixed (byte* source = fixture)
                Require(Native.LoadSnapshot(handle, source, (uint)fixture.Length) == Status.Ok, "raw load");
            Require(Native.GetState(handle, out State state) == Status.Ok, "raw state");
            byte[] small = Enumerable.Repeat((byte)0xaa, fixture.Length - 1).ToArray();
            fixed (byte* bytes = small)
                Require(Native.Save(handle, bytes, (uint)small.Length, out uint required) == Status.BufferTooSmall &&
                    required == fixture.Length, "required capacity");
            Require(small.All(b => b == 0xaa), "short save leaves destination untouched");
            Sample point = new() { X = 99, Y = 98, TimeUs = 97, Pressure = 0.75f, Flags = 1 };
            uint written = 123;
            Require(Native.ReadPoints(handle, state.Revision - 1, 0, 0, &point, 1, &written) == Status.StaleRevision, "raw stale read");
            Require(point.X == 99 && point.Y == 98 && point.TimeUs == 97 && point.Pressure == 0.75f &&
                point.Flags == 1 && written == 123, "stale read preserves all outputs");
            Require(Native.ReadPoints(handle, state.Revision, 0, 0, null, 0, &written) == Status.Ok &&
                written == 0, "zero capacity read");
        }
        finally { Require(Native.Destroy(handle) == Status.Ok, "raw destroy"); }
        Require(Native.GetState(handle, out _) == Status.InvalidHandle, "destroyed handle");
        Require(Native.Destroy(handle) == Status.InvalidHandle, "double native destroy");
    }

    private static void Lifecycle(string dll)
    {
        var disposed = new Engine(dll);
        disposed.Dispose(); disposed.Dispose();
        try { disposed.GetState(); throw new Exception("expected ObjectDisposedException"); }
        catch (ObjectDisposedException) { Require(true, "disposed access"); }

        using var racing = new Engine(dll);
        using var start = new ManualResetEventSlim();
        Task[] readers = Enumerable.Range(0, 4).Select(_ => Task.Run(() =>
        {
            start.Wait();
            for (int i = 0; i < 50; ++i)
            {
                try { Require(racing.GetState().HasActive == 0, "concurrent state"); }
                catch (ObjectDisposedException) { Require(true, "concurrent disposal"); }
            }
        })).ToArray();
        Task destroy = Task.Run(() => { start.Wait(); racing.Dispose(); });
        start.Set();
        Require(Task.WaitAll([.. readers, destroy], TimeSpan.FromSeconds(10)), "bounded lifecycle race");
        // 两个实例的状态不能互相污染。
        using var first = new Engine(dll);
        using var second = new Engine(dll);
        first.Begin(1, 0);
        Require(first.GetState().HasActive == 1 && second.GetState().HasActive == 0, "independent documents");
    }
}
