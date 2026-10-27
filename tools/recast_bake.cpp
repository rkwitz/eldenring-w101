// Bake a Recast polygon mesh from a triangle soup, for tools/er_navmesh.py (stand-alone; Recast is zlib-licensed).
//
// Input:  float32 triangle soup (x y z per vertex, 3 vertices per triangle, Y up).
// Output: text: "v x y z" per vertex (world units), "p n i0 .. in-1" per convex polygon, then its detail mesh:
//         "d n x y z .." (the polygon's vertices first, then interior samples) and "t a b c" per detail triangle.
// Build:  g++ -O2 -I<recast>/Recast/Include recast_bake.cpp <recast>/Recast/Source/*.cpp -o recast_bake
// Usage:  recast_bake in.bin out.txt cell_size cell_height agent_height agent_radius agent_climb max_slope_deg
//                     [max_verts_per_poly=6] [min_region_area=8] [merge_region_area=20] [max_edge_len_m=12]
#include <cstdio>
#include <cstdlib>
#include <cmath>
#include <vector>
#include "Recast.h"

int main(int argc, char** argv) {
    if (argc < 9) { fprintf(stderr, "usage: see source\n"); return 1; }
    FILE* f = fopen(argv[1], "rb");
    if (!f) { perror(argv[1]); return 1; }
    std::vector<float> verts;
    float buf[4096];
    size_t n;
    while ((n = fread(buf, sizeof(float), 4096, f)) > 0) verts.insert(verts.end(), buf, buf + n);
    fclose(f);
    const int nverts = (int)verts.size() / 3, ntris = nverts / 3;
    std::vector<int> tris(nverts);
    for (int i = 0; i < nverts; i++) tris[i] = i;

    rcConfig cfg = {};
    cfg.cs = atof(argv[3]);
    cfg.ch = atof(argv[4]);
    const float agentHeight = atof(argv[5]), agentRadius = atof(argv[6]), agentClimb = atof(argv[7]);
    cfg.walkableSlopeAngle = atof(argv[8]);
    cfg.maxVertsPerPoly = argc > 9 ? atoi(argv[9]) : 6;
    const int minRegion = argc > 10 ? atoi(argv[10]) : 8, mergeRegion = argc > 11 ? atoi(argv[11]) : 20;
    const float maxEdge = argc > 12 ? atof(argv[12]) : 12.0f;
    cfg.walkableHeight = (int)ceilf(agentHeight / cfg.ch);
    cfg.walkableClimb = (int)floorf(agentClimb / cfg.ch);
    cfg.walkableRadius = (int)ceilf(agentRadius / cfg.cs);
    cfg.maxEdgeLen = (int)(maxEdge / cfg.cs);
    cfg.maxSimplificationError = 1.3f;
    cfg.minRegionArea = minRegion * minRegion;
    cfg.mergeRegionArea = mergeRegion * mergeRegion;
    cfg.detailSampleDist = cfg.cs * 6;
    cfg.detailSampleMaxError = cfg.ch;
    rcCalcBounds(verts.data(), nverts, cfg.bmin, cfg.bmax);
    rcCalcGridSize(cfg.bmin, cfg.bmax, cfg.cs, &cfg.width, &cfg.height);
    fprintf(stderr, "grid %d x %d, %d triangles\n", cfg.width, cfg.height, ntris);

    rcContext ctx(false);
    rcHeightfield* hf = rcAllocHeightfield();
    if (!rcCreateHeightfield(&ctx, *hf, cfg.width, cfg.height, cfg.bmin, cfg.bmax, cfg.cs, cfg.ch)) return 2;
    std::vector<unsigned char> areas(ntris, 0);
    rcMarkWalkableTriangles(&ctx, cfg.walkableSlopeAngle, verts.data(), nverts, tris.data(), ntris, areas.data());
    if (!rcRasterizeTriangles(&ctx, verts.data(), nverts, tris.data(), areas.data(), ntris, *hf, cfg.walkableClimb)) return 3;
    rcFilterLowHangingWalkableObstacles(&ctx, cfg.walkableClimb, *hf);
    rcFilterLedgeSpans(&ctx, cfg.walkableHeight, cfg.walkableClimb, *hf);
    rcFilterWalkableLowHeightSpans(&ctx, cfg.walkableHeight, *hf);
    rcCompactHeightfield* chf = rcAllocCompactHeightfield();
    if (!rcBuildCompactHeightfield(&ctx, cfg.walkableHeight, cfg.walkableClimb, *hf, *chf)) return 4;
    rcFreeHeightField(hf);
    if (!rcErodeWalkableArea(&ctx, cfg.walkableRadius, *chf)) return 5;
    if (!rcBuildDistanceField(&ctx, *chf)) return 6;
    if (!rcBuildRegions(&ctx, *chf, 0, cfg.minRegionArea, cfg.mergeRegionArea)) return 7;
    rcContourSet* cset = rcAllocContourSet();
    if (!rcBuildContours(&ctx, *chf, cfg.maxSimplificationError, cfg.maxEdgeLen, *cset)) return 8;
    rcPolyMesh* pm = rcAllocPolyMesh();
    if (!rcBuildPolyMesh(&ctx, *cset, cfg.maxVertsPerPoly, *pm)) return 9;
    rcPolyMeshDetail* dm = rcAllocPolyMeshDetail();
    if (!rcBuildPolyMeshDetail(&ctx, *pm, *chf, cfg.detailSampleDist, cfg.detailSampleMaxError, *dm)) return 10;

    FILE* o = fopen(argv[2], "w");
    for (int i = 0; i < pm->nverts; i++) {
        const unsigned short* v = &pm->verts[i * 3];
        fprintf(o, "v %.4f %.4f %.4f\n", pm->bmin[0] + v[0] * pm->cs, pm->bmin[1] + v[1] * pm->ch, pm->bmin[2] + v[2] * pm->cs);
    }
    for (int i = 0; i < pm->npolys; i++) {
        const unsigned short* p = &pm->polys[i * pm->nvp * 2];
        int k = 0;
        while (k < pm->nvp && p[k] != RC_MESH_NULL_IDX) k++;
        fprintf(o, "p %d", k);
        for (int j = 0; j < k; j++) fprintf(o, " %d", p[j]);
        // detail triangles' vertex heights: "h" lines give this polygon's detail vertices (for height snapping)
        const unsigned int* m = &dm->meshes[i * 4];
        fprintf(o, "\nd %u", m[1]);
        for (unsigned int j = 0; j < m[1]; j++) {
            const float* dv = &dm->verts[(m[0] + j) * 3];
            fprintf(o, " %.3f %.3f %.3f", dv[0], dv[1], dv[2]);
        }
        // "t" lines: this polygon's detail triangles, as indices into its "d" vertices
        for (unsigned int j = 0; j < m[3]; j++) {
            const unsigned char* t = &dm->tris[(m[2] + j) * 4];
            fprintf(o, "\nt %d %d %d", t[0], t[1], t[2]);
        }
        fprintf(o, "\n");
    }
    fclose(o);
    fprintf(stderr, "%d vertices, %d polygons\n", pm->nverts, pm->npolys);
    return 0;
}
