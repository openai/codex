/*
** 2019-04-17
**
** The author disclaims copyright to this source code.  In place of
** a legal notice, here is a blessing:
**
**    May you do good and not evil.
**    May you find forgiveness for yourself and forgive others.
**    May you share freely, never taking more than you give.
**
******************************************************************************
**
** This file contains an implementation of two eponymous virtual tables,
** "sqlite_dbdata" and "sqlite_dbptr". Both modules require that the
** "sqlite_dbpage" eponymous virtual table be available.
**
** SQLITE_DBDATA:
**   sqlite_dbdata is used to extract data directly from a database b-tree
**   page and its associated overflow pages, bypassing the b-tree layer.
**   The table schema is equivalent to:
**
**     CREATE TABLE sqlite_dbdata(
**       pgno INTEGER,
**       cell INTEGER,
**       field INTEGER,
**       value ANY,
**       schema TEXT HIDDEN
**     );
**
**   IMPORTANT: THE VIRTUAL TABLE SCHEMA ABOVE IS SUBJECT TO CHANGE. IN THE
**   FUTURE NEW NON-HIDDEN COLUMNS MAY BE ADDED BETWEEN "value" AND
**   "schema".
**
**   Each page of the database is inspected. If it cannot be interpreted as
**   a b-tree page, or if it is a b-tree page containing 0 entries, the
**   sqlite_dbdata table contains no rows for that page.  Otherwise, the
**   table contains one row for each field in the record associated with
**   each cell on the page. For intkey b-trees, the key value is stored in
**   field -1.
**
**   For example, for the database:
**
**     CREATE TABLE t1(a, b);     -- root page is page 2
**     INSERT INTO t1(rowid, a, b) VALUES(5, 'v', 'five');
**     INSERT INTO t1(rowid, a, b) VALUES(10, 'x', 'ten');
**
**   the sqlite_dbdata table contains, as well as from entries related to
**   page 1, content equivalent to:
**
**     INSERT INTO sqlite_dbdata(pgno, cell, field, value) VALUES
**         (2, 0, -1, 5     ),
**         (2, 0,  0, 'v'   ),
**         (2, 0,  1, 'five'),
**         (2, 1, -1, 10    ),
**         (2, 1,  0, 'x'   ),
**         (2, 1,  1, 'ten' );
**
**   If database corruption is encountered, this module does not report an
**   error. Instead, it attempts to extract as much data as possible and
**   ignores the corruption.
**
** SQLITE_DBPTR:
**   The sqlite_dbptr table has the following schema:
**
**     CREATE TABLE sqlite_dbptr(
**       pgno INTEGER,
**       child INTEGER,
**       schema TEXT HIDDEN
**     );
**
**   It contains one entry for each b-tree pointer between a parent and
**   child page in the database.
*/

#if !defined(SQLITEINT_H)
#include "sqlite3.h"

typedef unsigned char u8;
typedef unsigned int u32;

#endif
#include <string.h>
#include <assert.h>

#ifndef SQLITE_OMIT_VIRTUALTABLE

#define DBDATA_PADDING_BYTES 100

typedef struct DbdataTable DbdataTable;
typedef struct DbdataCursor DbdataCursor;
typedef struct DbdataBuffer DbdataBuffer;

/*
** Buffer type.
*/
struct DbdataBuffer {
  u8 *aBuf;
  sqlite3_int64 nBuf;
};

/* Cursor object */
struct DbdataCursor {
  sqlite3_vtab_cursor base;       /* Base class.  Must be first */
  sqlite3_stmt *pStmt;            /* For fetching database pages */

  int iPgno;                      /* Current page number */
  u8 *aPage;                      /* Buffer containing page */
  int nPage;                      /* Size of aPage[] in bytes */
  int nCell;                      /* Number of cells on aPage[] */
  int iCell;                      /* Current cell number */
  int bOnePage;                   /* True to stop after one page */
  int szDb;
  sqlite3_int64 iRowid;

  /* Only for the sqlite_dbdata table */
  DbdataBuffer rec;
  sqlite3_int64 nRec;             /* Size of pRec[] in bytes */
  sqlite3_int64 nHdr;             /* Size of header in bytes */
  int iField;                     /* Current field number */
  u8 *pHdrPtr;
  u8 *pPtr;
  u32 enc;                        /* Text encoding */

  sqlite3_int64 iIntkey;          /* Integer key value */
};

/* Table object */
struct DbdataTable {
  sqlite3_vtab base;              /* Base class.  Must be first */
  sqlite3 *db;                    /* The database connection */
  sqlite3_stmt *pStmt;            /* For fetching database pages */
  int bPtr;                       /* True for sqlite3_dbptr table */
};

/* Column and schema definitions for sqlite_dbdata */
#define DBDATA_COLUMN_PGNO        0
#define DBDATA_COLUMN_CELL        1
#define DBDATA_COLUMN_FIELD       2
#define DBDATA_COLUMN_VALUE       3
#define DBDATA_COLUMN_SCHEMA      4
#define DBDATA_SCHEMA             \
      "CREATE TABLE x("           \
      "  pgno INTEGER,"           \
      "  cell INTEGER,"           \
      "  field INTEGER,"          \
      "  value ANY,"              \
      "  schema TEXT HIDDEN"      \
      ")"

/* Column and schema definitions for sqlite_dbptr */
#define DBPTR_COLUMN_PGNO         0
#define DBPTR_COLUMN_CHILD        1
#define DBPTR_COLUMN_SCHEMA       2
#define DBPTR_SCHEMA              \
      "CREATE TABLE x("           \
      "  pgno INTEGER,"           \
      "  child INTEGER,"          \
      "  schema TEXT HIDDEN"      \
      ")"

/*
** Ensure the buffer passed as the first argument is at least nMin bytes
** in size. If an error occurs while attempting to resize the buffer,
** SQLITE_NOMEM is returned. Otherwise, SQLITE_OK.
*/
static int dbdataBufferSize(DbdataBuffer *pBuf, sqlite3_int64 nMin){
  if( nMin>pBuf->nBuf ){
    sqlite3_int64 nNew = nMin+16384;
    u8 *aNew = (u8*)sqlite3_realloc64(pBuf->aBuf, nNew);

    if( aNew==0 ) return SQLITE_NOMEM;
    pBuf->aBuf = aNew;
    pBuf->nBuf = nNew;
  }
  return SQLITE_OK;
}

/*
** Release the allocation managed by buffer pBuf.
*/
static void dbdataBufferFree(DbdataBuffer *pBuf){
  sqlite3_free(pBuf->aBuf);
  memset(pBuf, 0, sizeof(*pBuf));
}

/*
** Connect to an sqlite_dbdata (pAux==0) or sqlite_dbptr (pAux!=0) virtual
** table.
*/
static int dbdataConnect(
  sqlite3 *db,
  void *pAux,
  int argc, const char *const*argv,
  sqlite3_vtab **ppVtab,
  char **pzErr
){
  DbdataTable *pTab = 0;
  int rc = sqlite3_declare_vtab(db, pAux ? DBPTR_SCHEMA : DBDATA_SCHEMA);

  (void)argc;
  (void)argv;
  (void)pzErr;
  sqlite3_vtab_config(db, SQLITE_VTAB_USES_ALL_SCHEMAS);
  if( rc==SQLITE_OK ){
    pTab = (DbdataTable*)sqlite3_malloc64(sizeof(DbdataTable));
    if( pTab==0 ){
      rc = SQLITE_NOMEM;
    }else{
      memset(pTab, 0, sizeof(DbdataTable));
      pTab->db = db;
      pTab->bPtr = (pAux!=0);
    }
  }

  *ppVtab = (sqlite3_vtab*)pTab;
  return rc;
}

/*
** Disconnect from or destroy a sqlite_dbdata or sqlite_dbptr virtual table.
*/
static int dbdataDisconnect(sqlite3_vtab *pVtab){
  DbdataTable *pTab = (DbdataTable*)pVtab;
  if( pTab ){
    sqlite3_finalize(pTab->pStmt);
    sqlite3_free(pVtab);
  }
  return SQLITE_OK;
}

/*
** This function interprets two types of constraints:
**
**       schema=?
**       pgno=?
**
** If neither are present, idxNum is set to 0. If schema=? is present,
** the 0x01 bit in idxNum is set. If pgno=? is present, the 0x02 bit
** in idxNum is set.
**
** If both parameters are present, schema is in position 0 and pgno in
** position 1.
*/
static int dbdataBestIndex(sqlite3_vtab *tab, sqlite3_index_info *pIdx){
  DbdataTable *pTab = (DbdataTable*)tab;
  int i;
  int iSchema = -1;
  int iPgno = -1;
  int colSchema = (pTab->bPtr ? DBPTR_COLUMN_SCHEMA : DBDATA_COLUMN_SCHEMA);

  for(i=0; i<pIdx->nConstraint; i++){
    struct sqlite3_index_constraint *p = &pIdx->aConstraint[i];
    if( p->op==SQLITE_INDEX_CONSTRAINT_EQ ){
      if( p->iColumn==colSchema ){
        if( p->usable==0 ) return SQLITE_CONSTRAINT;
        iSchema = i;
      }
      if( p->iColumn==DBDATA_COLUMN_PGNO && p->usable ){
        iPgno = i;
      }
    }
  }

  if( iSchema>=0 ){
    pIdx->aConstraintUsage[iSchema].argvIndex = 1;
    pIdx->aConstraintUsage[iSchema].omit = 1;
  }
  if( iPgno>=0 ){
    pIdx->aConstraintUsage[iPgno].argvIndex = 1 + (iSchema>=0);
    pIdx->aConstraintUsage[iPgno].omit = 1;
    pIdx->estimatedCost = 100;
    pIdx->estimatedRows =  50;

    if( pTab->bPtr==0 && pIdx->nOrderBy && pIdx->aOrderBy[0].desc==0 ){
      int iCol = pIdx->aOrderBy[0].iColumn;
      if( pIdx->nOrderBy==1 ){
        pIdx->orderByConsumed = (iCol==0 || iCol==1);
      }else if( pIdx->nOrderBy==2 && pIdx->aOrderBy[1].desc==0 && iCol==0 ){
        pIdx->orderByConsumed = (pIdx->aOrderBy[1].iColumn==1);
      }
    }

  }else{
    pIdx->estimatedCost = 100000000;
    pIdx->estimatedRows = 1000000000;
  }
  pIdx->idxNum = (iSchema>=0 ? 0x01 : 0x00) | (iPgno>=0 ? 0x02 : 0x00);
  return SQLITE_OK;
}

/*
** Open a new sqlite_dbdata or sqlite_dbptr cursor.
*/
static int dbdataOpen(sqlite3_vtab *pVTab, sqlite3_vtab_cursor **ppCursor){
  DbdataCursor *pCsr;

  pCsr = (DbdataCursor*)sqlite3_malloc64(sizeof(DbdataCursor));
  if( pCsr==0 ){
    return SQLITE_NOMEM;
  }else{
    memset(pCsr, 0, sizeof(DbdataCursor));
    pCsr->base.pVtab = pVTab;
  }

  *ppCursor = (sqlite3_vtab_cursor *)pCsr;
  return SQLITE_OK;
}

/*
** Restore a cursor object to the state it was in when first allocated
** by dbdataOpen().
*/
static void dbdataResetCursor(DbdataCursor *pCsr){
  DbdataTable *pTab = (DbdataTable*)(pCsr->base.pVtab);
  if( pTab->pStmt==0 ){
    pTab->pStmt = pCsr->pStmt;
  }else{
    sqlite3_finalize(pCsr->pStmt);
  }
  pCsr->pStmt = 0;
  pCsr->iPgno = 1;
  pCsr->iCell = 0;
  pCsr->iField = 0;
  pCsr->bOnePage = 0;
  sqlite3_free(pCsr->aPage);
  dbdataBufferFree(&pCsr->rec);
  pCsr->aPage = 0;
  pCsr->nRec = 0;
}

/*
** Close an sqlite_dbdata or sqlite_dbptr cursor.
*/
static int dbdataClose(sqlite3_vtab_cursor *pCursor){
  DbdataCursor *pCsr = (DbdataCursor*)pCursor;
  dbdataResetCursor(pCsr);
  sqlite3_free(pCsr);
  return SQLITE_OK;
}

/*
** Utility methods to decode 16 and 32-bit big-endian unsigned integers.
*/
static u32 get_uint16(unsigned char *a){
  return (a[0]<<8)|a[1];
}
static u32 get_uint32(unsigned char *a){
  return ((u32)a[0]<<24)
       | ((u32)a[1]<<16)
       | ((u32)a[2]<<8)
       | ((u32)a[3]);
}

/*
** Load page pgno from the database via the sqlite_dbpage virtual table.
** If successful, set (*ppPage) to point to a buffer containing the page
** data, (*pnPage) to the size of that buffer in bytes and return
** SQLITE_OK. In this case it is the responsibility of the caller to
** eventually free the buffer using sqlite3_free().
**
** Or, if an error occurs, set both (*ppPage) and (*pnPage) to 0 and
** return an SQLite error code.
*/
static int dbdataLoadPage(
  DbdataCursor *pCsr,             /* Cursor object */
  u32 pgno,                       /* Page number of page to load */
  u8 **ppPage,                    /* OUT: pointer to page buffer */
  int *pnPage                     /* OUT: Size of (*ppPage) in bytes */
){
  int rc2;
  int rc = SQLITE_OK;
  sqlite3_stmt *pStmt = pCsr->pStmt;

  *ppPage = 0;
  *pnPage = 0;
  if( pgno>0 ){
    sqlite3_bind_int64(pStmt, 2, pgno);
    if( SQLITE_ROW==sqlite3_step(pStmt) ){
      int nCopy = sqlite3_column_bytes(pStmt, 0);
      if( nCopy>0 ){
        u8 *pPage;
        pPage = (u8*)sqlite3_malloc64(nCopy + DBDATA_PADDING_BYTES);
        if( pPage==0 ){
          rc = SQLITE_NOMEM;
        }else{
          const u8 *pCopy = sqlite3_column_blob(pStmt, 0);
          memcpy(pPage, pCopy, nCopy);
          memset(&pPage[nCopy], 0, DBDATA_PADDING_BYTES);
        }
        *ppPage = pPage;
        *pnPage = nCopy;
      }
    }
    rc2 = sqlite3_reset(pStmt);
    if( rc==SQLITE_OK ) rc = rc2;
  }

  return rc;
}

/*
** Read a varint.  Put the value in *pVal and return the number of bytes.
*/
static int dbdataGetVarint(const u8 *z, sqlite3_int64 *pVal){
  sqlite3_uint64 u = 0;
  int i;
  for(i=0; i<8; i++){
    u = (u<<7) + (z[i]&0x7f);
    if( (z[i]&0x80)==0 ){ *pVal = (sqlite3_int64)u; return i+1; }
  }
  u = (u<<8) + (z[i]&0xff);
  *pVal = (sqlite3_int64)u;
  return 9;
}

/*
** Like dbdataGetVarint(), but set the output to 0 if it is less than 0
** or greater than 0xFFFFFFFF. This can be used for all varints in an
** SQLite database except for key values in intkey tables.
*/
static int dbdataGetVarintU32(const u8 *z, sqlite3_int64 *pVal){
  sqlite3_int64 val;
  int nRet = dbdataGetVarint(z, &val);
  if( val<0 || val>0xFFFFFFFF ) val = 0;
  *pVal = val;
  return nRet;
}

/*
** Return the number of bytes of space used by an SQLite value of type
** eType.
*/
static int dbdataValueBytes(int eType){
  switch( eType ){
    case 0: case 8: case 9:
    case 10: case 11:
      return 0;
    case 1:
      return 1;
    case 2:
      return 2;
    case 3:
      return 3;
    case 4:
      return 4;
    case 5:
      return 6;
    case 6:
    case 7:
      return 8;
    default:
      if( eType>0 ){
        return ((eType-12) / 2);
      }
      return 0;
  }
}

/*
** Load a value of type eType from buffer pData and use it to set the
** result of context object pCtx.
*/
static void dbdataValue(
  sqlite3_context *pCtx,
  u32 enc,
  int eType,
  u8 *pData,
  sqlite3_int64 nData
){
  if( eType>=0 ){
    if( dbdataValueBytes(eType)<=nData ){
      switch( eType ){
        case 0:
        case 10:
        case 11:
          sqlite3_result_null(pCtx);
          break;

        case 8:
          sqlite3_result_int(pCtx, 0);
          break;
        case 9:
          sqlite3_result_int(pCtx, 1);
          break;

        case 1: case 2: case 3: case 4: case 5: case 6: case 7: {
          sqlite3_uint64 v = (signed char)pData[0];
          pData++;
          switch( eType ){
            case 7:
            case 6:  v = (v<<16) + (pData[0]<<8) + pData[1];  pData += 2;
            case 5:  v = (v<<16) + (pData[0]<<8) + pData[1];  pData += 2;
            case 4:  v = (v<<8) + pData[0];  pData++;
            case 3:  v = (v<<8) + pData[0];  pData++;
            case 2:  v = (v<<8) + pData[0];  pData++;
          }

          if( eType==7 ){
            double r;
            memcpy(&r, &v, sizeof(r));
            sqlite3_result_double(pCtx, r);
          }else{
            sqlite3_result_int64(pCtx, (sqlite3_int64)v);
          }
          break;
        }

        default: {
          int n = ((eType-12) / 2);
          if( eType % 2 ){
            switch( enc ){
  #ifndef SQLITE_OMIT_UTF16
              case SQLITE_UTF16BE:
                sqlite3_result_text16be(pCtx, (void*)pData, n, SQLITE_TRANSIENT);
                break;
              case SQLITE_UTF16LE:
                sqlite3_result_text16le(pCtx, (void*)pData, n, SQLITE_TRANSIENT);
                break;
  #endif
              default:
                sqlite3_result_text(pCtx, (char*)pData, n, SQLITE_TRANSIENT);
                break;
            }
          }else{
            sqlite3_result_blob(pCtx, pData, n, SQLITE_TRANSIENT);
          }
        }
      }
    }else{
      if( eType==7 ){
        sqlite3_result_double(pCtx, 0.0);
      }else if( eType<7 ){
        sqlite3_result_int(pCtx, 0);
      }else if( eType%2 ){
        sqlite3_result_text(pCtx, "", 0, SQLITE_STATIC);
      }else{
        sqlite3_result_blob(pCtx, "", 0, SQLITE_STATIC);
      }
    }
  }
}

/* This macro is a copy of the MX_CELL() macro in the SQLite core. Given
** a page-size, it returns the maximum number of cells that may be present
** on the page.  */
#define DBDATA_MX_CELL(pgsz) ((pgsz-8)/6)

/* Maximum number of fields that may appear in a single record. This is
** the "hard-limit", according to comments in sqliteLimit.h. */
#define DBDATA_MX_FIELD 32676

/*
** Move an sqlite_dbdata or sqlite_dbptr cursor to the next entry.
*/
static int dbdataNext(sqlite3_vtab_cursor *pCursor){
  DbdataCursor *pCsr = (DbdataCursor*)pCursor;
  DbdataTable *pTab = (DbdataTable*)pCursor->pVtab;

  pCsr->iRowid++;
  while( 1 ){
    int rc;
    int iOff = (pCsr->iPgno==1 ? 100 : 0);
    int bNextPage = 0;

    if( pCsr->aPage==0 ){
      while( 1 ){
        if( pCsr->bOnePage==0 && pCsr->iPgno>pCsr->szDb ) return SQLITE_OK;
        rc = dbdataLoadPage(pCsr, pCsr->iPgno, &pCsr->aPage, &pCsr->nPage);
        if( rc!=SQLITE_OK ) return rc;
        if( pCsr->aPage && pCsr->nPage>=256 ) break;
        sqlite3_free(pCsr->aPage);
        pCsr->aPage = 0;
        if( pCsr->bOnePage ) return SQLITE_OK;
        pCsr->iPgno++;
      }

      assert( iOff+3+2<=pCsr->nPage );
      pCsr->iCell = pTab->bPtr ? -2 : 0;
      pCsr->nCell = get_uint16(&pCsr->aPage[iOff+3]);
      if( pCsr->nCell>DBDATA_MX_CELL(pCsr->nPage) ){
        pCsr->nCell = DBDATA_MX_CELL(pCsr->nPage);
      }
    }

    if( pTab->bPtr ){
      if( pCsr->aPage[iOff]!=0x02 && pCsr->aPage[iOff]!=0x05 ){
        pCsr->iCell = pCsr->nCell;
      }
      pCsr->iCell++;
      if( pCsr->iCell>=pCsr->nCell ){
        sqlite3_free(pCsr->aPage);
        pCsr->aPage = 0;
        if( pCsr->bOnePage ) return SQLITE_OK;
        pCsr->iPgno++;
      }else{
        return SQLITE_OK;
      }
    }else{
      /* If there is no record loaded, load it now. */
      assert( pCsr->rec.aBuf!=0 || pCsr->nRec==0 );
      if( pCsr->nRec==0 ){
        int bHasRowid = 0;
        int nPointer = 0;
        sqlite3_int64 nPayload = 0;
        sqlite3_int64 nHdr = 0;
        int iHdr;
        int U, X;
        int nLocal;

        switch( pCsr->aPage[iOff] ){
          case 0x02:
            nPointer = 4;
            break;
          case 0x0a:
            break;
          case 0x0d:
            bHasRowid = 1;
            break;
          default:
            /* This is not a b-tree page with records on it. Continue. */
            pCsr->iCell = pCsr->nCell;
            break;
        }

        if( pCsr->iCell>=pCsr->nCell ){
          bNextPage = 1;
        }else{
          int iCellPtr = iOff + 8 + nPointer + pCsr->iCell*2;

          if( iCellPtr>pCsr->nPage ){
            bNextPage = 1;
          }else{
            iOff = get_uint16(&pCsr->aPage[iCellPtr]);
          }

          /* For an interior node cell, skip past the child-page number */
          iOff += nPointer;

          /* Load the "byte of payload including overflow" field */
          if( bNextPage || iOff>pCsr->nPage || iOff<=iCellPtr ){
            bNextPage = 1;
          }else{
            iOff += dbdataGetVarintU32(&pCsr->aPage[iOff], &nPayload);
            if( nPayload>0x7fffff00 ) nPayload &= 0x3fff;
            if( nPayload==0 ) nPayload = 1;
          }

          /* If this is a leaf intkey cell, load the rowid */
          if( bHasRowid && !bNextPage && iOff<pCsr->nPage ){
            iOff += dbdataGetVarint(&pCsr->aPage[iOff], &pCsr->iIntkey);
          }

          /* Figure out how much data to read from the local page */
          U = pCsr->nPage;
          if( bHasRowid ){
            X = U-35;
          }else{
            X = ((U-12)*64/255)-23;
          }
          if( nPayload<=X ){
            nLocal = nPayload;
          }else{
            int M, K;
            M = ((U-12)*32/255)-23;
            K = M+((nPayload-M)%(U-4));
            if( K<=X ){
              nLocal = K;
            }else{
              nLocal = M;
            }
          }

          if( bNextPage || nLocal+iOff>pCsr->nPage ){
            bNextPage = 1;
          }else{

            /* Allocate space for payload. And a bit more to catch small buffer
            ** overruns caused by attempting to read a varint or similar from
            ** near the end of a corrupt record.  */
            rc = dbdataBufferSize(&pCsr->rec, nPayload+DBDATA_PADDING_BYTES);
            if( rc!=SQLITE_OK ) return rc;
            assert( pCsr->rec.aBuf!=0 );
            assert( nPayload!=0 );

            /* Load the nLocal bytes of payload */
            memcpy(pCsr->rec.aBuf, &pCsr->aPage[iOff], nLocal);
            iOff += nLocal;

            /* Load content from overflow pages */
            if( nPayload>nLocal ){
              sqlite3_int64 nRem = nPayload - nLocal;
              u32 pgnoOvfl = get_uint32(&pCsr->aPage[iOff]);
              while( nRem>0 ){
                u8 *aOvfl = 0;
                int nOvfl = 0;
                int nCopy;
                rc = dbdataLoadPage(pCsr, pgnoOvfl, &aOvfl, &nOvfl);
                assert( rc!=SQLITE_OK || aOvfl==0 || nOvfl==pCsr->nPage );
                if( rc!=SQLITE_OK ) return rc;
                if( aOvfl==0 ) break;

                nCopy = U-4;
                if( nCopy>nRem ) nCopy = nRem;
                memcpy(&pCsr->rec.aBuf[nPayload-nRem], &aOvfl[4], nCopy);
                nRem -= nCopy;

                pgnoOvfl = get_uint32(aOvfl);
                sqlite3_free(aOvfl);
              }
              nPayload -= nRem;
            }
            memset(&pCsr->rec.aBuf[nPayload], 0, DBDATA_PADDING_BYTES);
            pCsr->nRec = nPayload;

            iHdr = dbdataGetVarintU32(pCsr->rec.aBuf, &nHdr);
            if( nHdr>nPayload ) nHdr = 0;
            pCsr->nHdr = nHdr;
            pCsr->pHdrPtr = &pCsr->rec.aBuf[iHdr];
            pCsr->pPtr = &pCsr->rec.aBuf[pCsr->nHdr];
            pCsr->iField = (bHasRowid ? -1 : 0);
          }
        }
      }else{
        pCsr->iField++;
        if( pCsr->iField>0 ){
          sqlite3_int64 iType;
          if( pCsr->pHdrPtr>=&pCsr->rec.aBuf[pCsr->nRec]
           || pCsr->iField>=DBDATA_MX_FIELD
          ){
            bNextPage = 1;
          }else{
            int szField = 0;
            pCsr->pHdrPtr += dbdataGetVarintU32(pCsr->pHdrPtr, &iType);
            szField = dbdataValueBytes(iType);
            if( (pCsr->nRec - (pCsr->pPtr - pCsr->rec.aBuf))<szField ){
              pCsr->pPtr = &pCsr->rec.aBuf[pCsr->nRec];
            }else{
              pCsr->pPtr += szField;
            }
          }
        }
      }

      if( bNextPage ){
        sqlite3_free(pCsr->aPage);
        pCsr->aPage = 0;
        pCsr->nRec = 0;
        if( pCsr->bOnePage ) return SQLITE_OK;
        pCsr->iPgno++;
      }else{
        if( pCsr->iField<0 || pCsr->pHdrPtr<&pCsr->rec.aBuf[pCsr->nHdr] ){
          return SQLITE_OK;
        }

        /* Advance to the next cell. The next iteration of the loop will load
        ** the record and so on. */
        pCsr->nRec = 0;
        pCsr->iCell++;
      }
    }
  }

  assert( !"can't get here" );
  return SQLITE_OK;
}

/*
** Return true if the cursor is at EOF.
*/
static int dbdataEof(sqlite3_vtab_cursor *pCursor){
  DbdataCursor *pCsr = (DbdataCursor*)pCursor;
  return pCsr->aPage==0;
}

/*
** Return true if nul-terminated string zSchema ends in "()". Or false
** otherwise.
*/
static int dbdataIsFunction(const char *zSchema){
  size_t n = strlen(zSchema);
  if( n>2 && zSchema[n-2]=='(' && zSchema[n-1]==')' ){
    return (int)n-2;
  }
  return 0;
}

/*
** Determine the size in pages of database zSchema (where zSchema is
** "main", "temp" or the name of an attached database) and set
** pCsr->szDb accordingly. If successful, return SQLITE_OK. Otherwise,
** an SQLite error code.
*/
static int dbdataDbsize(DbdataCursor *pCsr, const char *zSchema){
  DbdataTable *pTab = (DbdataTable*)pCsr->base.pVtab;
  char *zSql = 0;
  int rc, rc2;
  int nFunc = 0;
  sqlite3_stmt *pStmt = 0;

  if( (nFunc = dbdataIsFunction(zSchema))>0 ){
    zSql = sqlite3_mprintf("SELECT %.*s(0)", nFunc, zSchema);
  }else{
    zSql = sqlite3_mprintf("PRAGMA %Q.page_count", zSchema);
  }
  if( zSql==0 ) return SQLITE_NOMEM;

  rc = sqlite3_prepare_v2(pTab->db, zSql, -1, &pStmt, 0);
  sqlite3_free(zSql);
  if( rc==SQLITE_OK && sqlite3_step(pStmt)==SQLITE_ROW ){
    pCsr->szDb = sqlite3_column_int(pStmt, 0);
  }
  rc2 = sqlite3_finalize(pStmt);
  if( rc==SQLITE_OK ) rc = rc2;
  return rc;
}

/*
** Attempt to figure out the encoding of the database by retrieving page 1
** and inspecting the header field. If successful, set the pCsr->enc variable
** and return SQLITE_OK. Otherwise, return an SQLite error code.
*/
static int dbdataGetEncoding(DbdataCursor *pCsr){
  int rc = SQLITE_OK;
  int nPg1 = 0;
  u8 *aPg1 = 0;
  rc = dbdataLoadPage(pCsr, 1, &aPg1, &nPg1);
  if( rc==SQLITE_OK && nPg1>=(56+4) ){
    pCsr->enc = get_uint32(&aPg1[56]);
  }
  sqlite3_free(aPg1);
  return rc;
}


/*
** xFilter method for sqlite_dbdata and sqlite_dbptr.
*/
static int dbdataFilter(
  sqlite3_vtab_cursor *pCursor,
  int idxNum, const char *idxStr,
  int argc, sqlite3_value **argv
){
  DbdataCursor *pCsr = (DbdataCursor*)pCursor;
  DbdataTable *pTab = (DbdataTable*)pCursor->pVtab;
  int rc = SQLITE_OK;
  const char *zSchema = "main";
  (void)idxStr;
  (void)argc;

  dbdataResetCursor(pCsr);
  assert( pCsr->iPgno==1 );
  if( idxNum & 0x01 ){
    zSchema = (const char*)sqlite3_value_text(argv[0]);
    if( zSchema==0 ) zSchema = "";
  }
  if( idxNum & 0x02 ){
    pCsr->iPgno = sqlite3_value_int(argv[(idxNum & 0x01)]);
    pCsr->bOnePage = 1;
  }else{
    rc = dbdataDbsize(pCsr, zSchema);
  }

  if( rc==SQLITE_OK ){
    int nFunc = 0;
    if( pTab->pStmt ){
      pCsr->pStmt = pTab->pStmt;
      pTab->pStmt = 0;
    }else if( (nFunc = dbdataIsFunction(zSchema))>0 ){
      char *zSql = sqlite3_mprintf("SELECT %.*s(?2)", nFunc, zSchema);
      if( zSql==0 ){
        rc = SQLITE_NOMEM;
      }else{
        rc = sqlite3_prepare_v2(pTab->db, zSql, -1, &pCsr->pStmt, 0);
        sqlite3_free(zSql);
      }
    }else{
      rc = sqlite3_prepare_v2(pTab->db,
          "SELECT data FROM sqlite_dbpage(?) WHERE pgno=?", -1,
          &pCsr->pStmt, 0
      );
    }
  }
  if( rc==SQLITE_OK ){
    rc = sqlite3_bind_text(pCsr->pStmt, 1, zSchema, -1, SQLITE_TRANSIENT);
  }

  /* Try to determine the encoding of the db by inspecting the header
  ** field on page 1. */
  if( rc==SQLITE_OK ){
    rc = dbdataGetEncoding(pCsr);
  }

  if( rc!=SQLITE_OK ){
    pTab->base.zErrMsg = sqlite3_mprintf("%s", sqlite3_errmsg(pTab->db));
  }

  if( rc==SQLITE_OK ){
    rc = dbdataNext(pCursor);
  }
  return rc;
}

/*
** Return a column for the sqlite_dbdata or sqlite_dbptr table.
*/
static int dbdataColumn(
  sqlite3_vtab_cursor *pCursor,
  sqlite3_context *ctx,
  int i
){
  DbdataCursor *pCsr = (DbdataCursor*)pCursor;
  DbdataTable *pTab = (DbdataTable*)pCursor->pVtab;
  if( pTab->bPtr ){
    switch( i ){
      case DBPTR_COLUMN_PGNO:
        sqlite3_result_int64(ctx, pCsr->iPgno);
        break;
      case DBPTR_COLUMN_CHILD: {
        int iOff = pCsr->iPgno==1 ? 100 : 0;
        if( pCsr->iCell<0 ){
          iOff += 8;
        }else{
          iOff += 12 + pCsr->iCell*2;
          if( iOff>pCsr->nPage ) return SQLITE_OK;
          iOff = get_uint16(&pCsr->aPage[iOff]);
        }
        if( iOff<=pCsr->nPage ){
          sqlite3_result_int64(ctx, get_uint32(&pCsr->aPage[iOff]));
        }
        break;
      }
    }
  }else{
    switch( i ){
      case DBDATA_COLUMN_PGNO:
        sqlite3_result_int64(ctx, pCsr->iPgno);
        break;
      case DBDATA_COLUMN_CELL:
        sqlite3_result_int(ctx, pCsr->iCell);
        break;
      case DBDATA_COLUMN_FIELD:
        sqlite3_result_int(ctx, pCsr->iField);
        break;
      case DBDATA_COLUMN_VALUE: {
        if( pCsr->iField<0 ){
          sqlite3_result_int64(ctx, pCsr->iIntkey);
        }else if( &pCsr->rec.aBuf[pCsr->nRec] >= pCsr->pPtr ){
          sqlite3_int64 iType;
          dbdataGetVarintU32(pCsr->pHdrPtr, &iType);
          dbdataValue(
              ctx, pCsr->enc, iType, pCsr->pPtr,
              &pCsr->rec.aBuf[pCsr->nRec] - pCsr->pPtr
          );
        }
        break;
      }
    }
  }
  return SQLITE_OK;
}

/*
** Return the rowid for an sqlite_dbdata or sqlite_dptr table.
*/
static int dbdataRowid(sqlite3_vtab_cursor *pCursor, sqlite_int64 *pRowid){
  DbdataCursor *pCsr = (DbdataCursor*)pCursor;
  *pRowid = pCsr->iRowid;
  return SQLITE_OK;
}


/*
** Invoke this routine to register the "sqlite_dbdata" virtual table module
*/
static int sqlite3DbdataRegister(sqlite3 *db){
  static sqlite3_module dbdata_module = {
    0,                            /* iVersion */
    0,                            /* xCreate */
    dbdataConnect,                /* xConnect */
    dbdataBestIndex,              /* xBestIndex */
    dbdataDisconnect,             /* xDisconnect */
    0,                            /* xDestroy */
    dbdataOpen,                   /* xOpen - open a cursor */
    dbdataClose,                  /* xClose - close a cursor */
    dbdataFilter,                 /* xFilter - configure scan constraints */
    dbdataNext,                   /* xNext - advance a cursor */
    dbdataEof,                    /* xEof - check for end of scan */
    dbdataColumn,                 /* xColumn - read data */
    dbdataRowid,                  /* xRowid - read data */
    0,                            /* xUpdate */
    0,                            /* xBegin */
    0,                            /* xSync */
    0,                            /* xCommit */
    0,                            /* xRollback */
    0,                            /* xFindMethod */
    0,                            /* xRename */
    0,                            /* xSavepoint */
    0,                            /* xRelease */
    0,                            /* xRollbackTo */
    0,                            /* xShadowName */
    0                             /* xIntegrity */
  };

  int rc = sqlite3_create_module(db, "sqlite_dbdata", &dbdata_module, 0);
  if( rc==SQLITE_OK ){
    rc = sqlite3_create_module(db, "sqlite_dbptr", &dbdata_module, (void*)1);
  }
  return rc;
}

#ifdef _WIN32
__declspec(dllexport)
#endif
int sqlite3_dbdata_init(
  sqlite3 *db,
  char **pzErrMsg,
  const sqlite3_api_routines *pApi
){
  (void)pzErrMsg;
  return sqlite3DbdataRegister(db);
}

#endif /* ifndef SQLITE_OMIT_VIRTUALTABLE */
