package zpl_test

import (
	"context"
	"fmt"

	zpl "github.com/codyps/zpl/zpl-go"
)

// The same application code runs under each of the four backend build tags.
func ExampleEngine_Render() {
	ctx := context.Background()
	engine, err := zpl.New(ctx)
	if err != nil {
		panic(err)
	}
	defer engine.Close(ctx)
	result, err := engine.Render(ctx, zpl.Request{
		Input:   []byte("^XA^FO5,5^GB30,20,2^FS^XZ"),
		Format:  zpl.PNG,
		Profile: zpl.Specification,
		Width:   64,
		Height:  64,
	})
	if err != nil {
		panic(err)
	}
	fmt.Printf("%dx%d; %d label\n", result.Width, result.Height, result.Labels)
	// Output: 64x64; 1 label
}
