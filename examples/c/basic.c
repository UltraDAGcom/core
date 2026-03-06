/*
 * TinyDAG C FFI Example
 *
 * Build the library:
 *   cargo build --release -p tinydag-ffi
 *
 * Compile this example:
 *   gcc -o basic examples/c/basic.c \
 *       -Ibindings/tinydag-ffi/include \
 *       -Ltarget/release -ltinydag_ffi
 *
 * Run:
 *   LD_LIBRARY_PATH=target/release ./basic
 */

#include <stdio.h>
#include <string.h>
#include "tinydag.h"

int main(void) {
    /* Version */
    printf("TinyDAG version: %s\n", tinydag_version());

    /* Blake3 hashing */
    const char* data = "hello tinydag";
    uint8_t hash[32];
    tinydag_blake3((const uint8_t*)data, strlen(data), hash);

    printf("Blake3 hash: ");
    for (int i = 0; i < 32; i++) {
        printf("%02x", hash[i]);
    }
    printf("\n");

    /* Build a graph */
    GraphBuilderHandle* builder = tinydag_graph_builder_new();

    uint8_t sensor_id[32], filter_id[32];
    tinydag_graph_builder_add_node(builder, "sensor", "sensor_v1", sensor_id);
    tinydag_graph_builder_add_node(builder, "filter", "filter_v1", filter_id);
    tinydag_graph_builder_add_edge(builder, sensor_id, filter_id);

    ValidatedGraphHandle* graph = tinydag_graph_builder_build(builder);
    if (graph == NULL) {
        printf("Graph validation failed!\n");
        return 1;
    }

    printf("Graph nodes: %zu\n", tinydag_graph_node_count(graph));

    /* Execute */
    ExecutionResultHandle* result = tinydag_graph_execute(graph);
    if (result == NULL) {
        printf("Execution failed!\n");
        tinydag_graph_free(graph);
        return 1;
    }

    printf("Execution complete.\n");

    /* Cleanup */
    tinydag_result_free(result);
    tinydag_graph_free(graph);

    /* Values */
    TinyValueHandle* val = tinydag_value_int(42);
    tinydag_value_free(val);

    return 0;
}
